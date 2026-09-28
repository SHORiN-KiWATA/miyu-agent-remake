//! 发一次请求（`docs/designs/05-内核接口.md` 第七节「HTTP 执行器」）：照剧本回的假服务器，查发出去的、
//! 读回来的、空闲超时、打断、连不上、说到一半断开。

mod support;

use std::time::Duration;

use miyu_drivers::openai_chat::Decoder;
use miyu_http::testkit::{Piece, Reply, Server};
use miyu_http::{Attempt, Endpoint, Outcome, Progress, Proxy, client, send};
use miyu_kernel::accumulate::Delta;
use miyu_kernel::event::ErrorClass;
use miyu_kernel::id::ContentHash;
use support::{BODY, driver, sample};

/// 发一次，收集交出来的和收场。
async fn run(
    endpoint: &Endpoint,
    idle: Duration,
    cancel: impl std::future::Future<Output = ()> + Send,
) -> (Vec<Progress>, Outcome) {
    let client = client(Proxy::Off).expect("造得出客户端");
    let driver = driver();
    let mut progress = Vec::new();
    let outcome = send(
        Attempt {
            client: &client,
            endpoint,
            driver: &driver,
            body: BODY,
            path: "/chat/completions",
            idle,
        },
        cancel,
        |step| progress.push(step),
    )
    .await;
    (progress, outcome)
}

fn never() -> std::future::Pending<()> {
    std::future::pending()
}

fn deltas(progress: &[Progress]) -> Vec<Delta> {
    progress
        .iter()
        .filter_map(|step| match step {
            Progress::Delta(delta) => Some(delta.clone()),
            Progress::Sent { .. } => None,
        })
        .collect()
}

fn class(outcome: &Outcome) -> Option<ErrorClass> {
    match outcome {
        Outcome::Ended { error, .. } => error.as_ref().map(|error| error.error.class.clone()),
        Outcome::Cancelled => None,
    }
}

#[tokio::test]
async fn a_stream_comes_back_as_deltas() {
    let stream = sample("deepseek-reasoning-tools");
    let (first, rest) = stream.split_at(300);
    let (second, third) = rest.split_at(500);
    let server = Server::start(vec![Reply::stream(vec![
        Piece::Bytes(first.to_vec()),
        Piece::Wait(Duration::from_millis(20)),
        Piece::Bytes(second.to_vec()),
        Piece::Bytes(third.to_vec()),
    ])])
    .await;
    let endpoint = Endpoint::new(&server.base_url, "sk-test");
    let (progress, outcome) = run(&endpoint, Duration::from_secs(5), never()).await;
    // 先报发出去了，带着请求字节的哈希。
    assert_eq!(
        progress.first(),
        Some(&Progress::Sent {
            request: ContentHash::of(BODY)
        })
    );
    // 增量和直接解这份样本的一样，收块的也在。
    let mut decoder = Decoder::new();
    let mut expected = decoder.feed(&stream);
    let ending = decoder.finish();
    expected.extend(ending.deltas);
    assert_eq!(deltas(&progress), expected);
    assert_eq!(
        outcome,
        Outcome::Ended {
            usage: ending.usage,
            error: None
        }
    );
}

#[tokio::test]
async fn the_request_is_what_the_driver_encoded() {
    let server = Server::start(vec![Reply::stream(vec![Piece::Bytes(sample(
        "openai-text",
    ))])])
    .await;
    let endpoint =
        Endpoint::new(format!("{}/", server.base_url), "sk-test").with_header("X-Title", "miyu");
    let (_, outcome) = run(&endpoint, Duration::from_secs(5), never()).await;
    assert!(matches!(outcome, Outcome::Ended { error: None, .. }));
    let received = server.received();
    assert_eq!(received.len(), 1);
    let request = &received[0];
    assert_eq!(request.method, "POST");
    // 地址后面多一个斜杠也不会变成两个。
    assert_eq!(request.path, "/v1/chat/completions");
    assert_eq!(request.header("authorization"), Some("Bearer sk-test"));
    assert_eq!(request.header("content-type"), Some("application/json"));
    assert_eq!(request.header("accept"), Some("text/event-stream"));
    assert!(
        request
            .header("user-agent")
            .is_some_and(|agent| agent.starts_with("miyu/"))
    );
    assert_eq!(request.header("x-title"), Some("miyu"));
    assert_eq!(request.body, BODY);
}

#[tokio::test]
async fn it_stops_reading_once_the_reply_is_done() {
    // 见到 [DONE] 以后服务器不关连接、停住：客户端马上收场，不等空闲超时。
    let server = Server::start(vec![Reply::stream(vec![
        Piece::Bytes(sample("openai-text")),
        Piece::Stall,
    ])])
    .await;
    let endpoint = Endpoint::new(&server.base_url, "sk-test");
    let started = tokio::time::Instant::now();
    let (_, outcome) = run(&endpoint, Duration::from_secs(10), never()).await;
    assert!(
        matches!(outcome, Outcome::Ended { error: None, .. }),
        "{outcome:?}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "见到 [DONE] 就停"
    );
}

#[tokio::test]
async fn an_http_error_is_classified() {
    let server = Server::start(vec![Reply::error(
        429,
        &[("Retry-After", "7")],
        r#"{"error":{"message":"Rate limit reached"}}"#,
    )])
    .await;
    let endpoint = Endpoint::new(&server.base_url, "sk-test");
    let (progress, outcome) = run(&endpoint, Duration::from_secs(5), never()).await;
    assert!(deltas(&progress).is_empty());
    let Outcome::Ended {
        error: Some(error), ..
    } = outcome
    else {
        panic!("应该出错：{outcome:?}");
    };
    assert_eq!(error.error.class, ErrorClass::RateLimited);
    assert_eq!(error.retry_after_ms, Some(7000));
    assert_eq!(error.error.message, "HTTP 429: Rate limit reached");
}

#[tokio::test]
async fn a_stall_times_out() {
    let stream = sample("openai-text");
    let first_event = stream
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .expect("有第一条")
        + 4;
    let second_event = first_event
        + stream[first_event..]
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .expect("有第二条")
        + 4;
    let server = Server::start(vec![Reply::stream(vec![
        Piece::Bytes(stream[..second_event].to_vec()),
        Piece::Stall,
    ])])
    .await;
    let endpoint = Endpoint::new(&server.base_url, "sk-test");
    let (progress, outcome) = run(&endpoint, Duration::from_millis(200), never()).await;
    // 停住之前的增量照样交出来了：第二条里的「你好」。
    assert!(
        deltas(&progress)
            .iter()
            .any(|delta| matches!(delta, Delta::Text { text, .. } if text == "你好"))
    );
    let Outcome::Ended {
        error: Some(error), ..
    } = outcome
    else {
        panic!("应该超时：{outcome:?}");
    };
    assert_eq!(error.error.class, ErrorClass::Retryable);
    assert!(
        error.error.message.contains("空闲超时"),
        "{}",
        error.error.message
    );
}

#[tokio::test]
async fn cancelling_stops_right_away() {
    let mut server = Server::start(vec![Reply::stream(vec![Piece::Stall])]).await;
    let endpoint = Endpoint::new(&server.base_url, "sk-test");
    // 等假服务器回完了头、停住了再叫停：测的是读到一半打断。以前定死在 100 毫秒，慢的机器上那一刻
    // 可能还在连、还在写请求，客户端在后台把连接建完放进池子里，假服务器就看不到断开（macOS 上偶发，
    // 施工 3-5 再补）。
    let cancel = server.wait_stalled(1);
    let started = tokio::time::Instant::now();
    let (_, outcome) = run(&endpoint, Duration::from_secs(30), cancel).await;
    assert_eq!(outcome, Outcome::Cancelled);
    assert!(started.elapsed() < Duration::from_secs(5), "马上停");
    // 假服务器看到连接断了。
    server.wait_closed(1).await;
}

#[tokio::test]
async fn nobody_listening_is_retryable() {
    // 先占一个端口再放掉：上面没人听。
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("绑得上");
    let port = listener.local_addr().expect("有地址").port();
    drop(listener);
    // 有的供应商把 key 放在地址里。
    let endpoint = Endpoint::new(format!("http://127.0.0.1:{port}/sk-in-path/v1"), "sk-test");
    let (progress, outcome) = run(&endpoint, Duration::from_secs(5), never()).await;
    assert!(matches!(progress.first(), Some(Progress::Sent { .. })));
    let Outcome::Ended {
        error: Some(error), ..
    } = outcome
    else {
        panic!("应该出错：{outcome:?}");
    };
    assert_eq!(error.error.class, ErrorClass::Retryable);
    // 原话里没有地址（施工 4-9 再补三下：原来 reqwest 的错带着整个地址）。
    let message = &error.error.message;
    assert!(!message.contains("sk-in-path"), "{message}");
    assert!(!message.contains("://"), "{message}");
}

#[tokio::test]
async fn a_connection_dropped_mid_reply_is_retryable() {
    let stream = sample("openai-text");
    let server = Server::start(vec![Reply::stream(vec![
        Piece::Bytes(stream[..stream.len() / 2].to_vec()),
        Piece::Drop,
    ])])
    .await;
    let endpoint = Endpoint::new(&server.base_url, "sk-test");
    let (_, outcome) = run(&endpoint, Duration::from_secs(5), never()).await;
    assert_eq!(class(&outcome), Some(ErrorClass::Retryable));
}

#[tokio::test]
async fn a_body_cut_short_says_how_the_connection_broke() {
    // 声明了长度，没写够就断开：客户端读到的是出错，不是正常读完。
    let stream = sample("openai-text");
    let mut reply = Reply::stream(vec![
        Piece::Bytes(stream[..stream.len() / 2].to_vec()),
        Piece::Drop,
    ]);
    reply
        .headers
        .push(("Content-Length".to_string(), stream.len().to_string()));
    let server = Server::start(vec![reply]).await;
    let endpoint = Endpoint::new(&server.base_url, "sk-test");
    let (_, outcome) = run(&endpoint, Duration::from_secs(5), never()).await;
    let Outcome::Ended {
        error: Some(error), ..
    } = outcome
    else {
        panic!("应该出错：{outcome:?}");
    };
    assert_eq!(error.error.class, ErrorClass::Retryable);
    assert!(
        error.error.message.starts_with("连接断了："),
        "{}",
        error.error.message
    );
}

#[tokio::test]
async fn it_counts_as_done_when_only_done_is_missing() {
    // `finish_reason`、用量都到了，只差 `[DONE]` 就停住：算说完，增量、用量照交（施工 4-9 再补三下：
    // 原来算空闲超时出错，重来一遍）。
    let stream = sample("openai-text");
    let done = stream
        .windows(12)
        .position(|window| window == b"data: [DONE]")
        .expect("有 [DONE]");
    let server = Server::start(vec![Reply::stream(vec![
        Piece::Bytes(stream[..done].to_vec()),
        Piece::Stall,
    ])])
    .await;
    let endpoint = Endpoint::new(&server.base_url, "sk-test");
    let (progress, outcome) = run(&endpoint, Duration::from_millis(200), never()).await;
    let mut decoder = Decoder::new();
    let mut expected = decoder.feed(&stream);
    let ending = decoder.finish();
    expected.extend(ending.deltas);
    assert_eq!(deltas(&progress), expected);
    assert!(ending.usage.is_some());
    assert_eq!(
        outcome,
        Outcome::Ended {
            usage: ending.usage,
            error: None
        }
    );
}

#[tokio::test]
async fn a_rate_limit_in_the_stream_says_how_long_to_wait() {
    let reply = "data: {\"error\":{\"message\":\"Rate limit reached. Please try again in 1.5s\",\
                 \"type\":\"rate_limit_error\",\"code\":429}}\n\n";
    let server = Server::start(vec![Reply::stream(vec![Piece::Bytes(
        reply.as_bytes().to_vec(),
    )])])
    .await;
    let endpoint = Endpoint::new(&server.base_url, "sk-test");
    let (_, outcome) = run(&endpoint, Duration::from_secs(5), never()).await;
    let Outcome::Ended {
        error: Some(error), ..
    } = outcome
    else {
        panic!("应该出错：{outcome:?}");
    };
    assert_eq!(error.error.class, ErrorClass::RateLimited);
    // 供应商说的 1.5 秒交回去，内核照这个等（施工 4-9 再补三下：原来丢掉，内核照自己的退避）。
    assert_eq!(error.retry_after_ms, Some(1500));
}

#[tokio::test]
async fn a_header_of_the_same_name_replaces_the_default() {
    let server = Server::start(vec![Reply::stream(vec![Piece::Bytes(sample(
        "openai-text",
    ))])])
    .await;
    let endpoint = Endpoint::new(&server.base_url, "sk-test")
        .with_header("accept", "application/x-ndjson")
        .with_header("X-Tag", "a")
        .with_header("X-Tag", "b");
    let (_, outcome) = run(&endpoint, Duration::from_secs(5), never()).await;
    assert!(
        matches!(outcome, Outcome::Ended { error: None, .. }),
        "{outcome:?}"
    );
    let received = server.received();
    let values = |name: &str| -> Vec<String> {
        received[0]
            .headers
            .iter()
            .filter(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.clone())
            .collect()
    };
    // 另配的 `Accept` 换掉默认的：只有一个，是另配的那个（施工 4-9 再补三下：原来发两个）。
    assert_eq!(values("accept"), ["application/x-ndjson"]);
    // 另配的几个同名的，照先后都发。
    assert_eq!(values("x-tag"), ["a", "b"]);
}

#[tokio::test]
async fn a_bad_address_or_header_is_other_and_not_sent() {
    let server = Server::start(vec![
        Reply::stream(vec![Piece::Bytes(sample("openai-text"))]);
        2
    ])
    .await;
    let endpoints = [
        Endpoint::new("not a url", "sk-test"),
        Endpoint::new("ftp://127.0.0.1/v1", "sk-test"),
        Endpoint::new(&server.base_url, "sk-test").with_header("bad name", "x"),
        Endpoint::new(&server.base_url, "sk-test").with_header("X-Tag", "sk-hidden\n"),
    ];
    for endpoint in &endpoints {
        let (progress, outcome) = run(endpoint, Duration::from_secs(5), never()).await;
        // 照样先报发出去了。
        assert!(matches!(progress.first(), Some(Progress::Sent { .. })));
        let Outcome::Ended {
            error: Some(error), ..
        } = outcome
        else {
            panic!("应该出错：{endpoint:?} {outcome:?}");
        };
        // 造不出请求，重来也一样：`other`，不重试（施工 4-9 再补三下：原来地址写坏了是 `retryable`）。
        assert_eq!(error.error.class, ErrorClass::Unclassified, "{endpoint:?}");
        let message = &error.error.message;
        assert!(message.starts_with("地址或者头写得不对："), "{message}");
        // 不带地址，不带头的值（值也可能是密钥）。
        assert!(!message.contains("://"), "{message}");
        assert!(!message.contains("sk-hidden"), "{message}");
    }
    // 一个都没发到服务器。
    assert!(server.received().is_empty());
}

#[test]
fn the_address_is_printed_as_its_host_only() {
    // 有的供应商把 key 放在地址的路径、参数里（施工 4-9 再补三下：原来打出整个地址）。
    let endpoint = Endpoint::new(
        "https://api.example.com/sk-in-path/v1?key=sk-in-query",
        "sk",
    );
    let printed = format!("{endpoint:?}");
    assert!(printed.contains("api.example.com"), "{printed}");
    assert!(!printed.contains("sk-in-path"), "{printed}");
    assert!(!printed.contains("sk-in-query"), "{printed}");
}

#[test]
fn the_key_is_not_printed() {
    let endpoint = Endpoint::new("https://api.deepseek.com", "sk-secret-123")
        .with_header("X-Api-Key", "another-secret");
    let printed = format!("{endpoint:?}");
    assert!(!printed.contains("sk-secret-123"), "{printed}");
    assert!(!printed.contains("another-secret"), "{printed}");
    assert!(printed.contains("***"));
    assert!(printed.contains("X-Api-Key"));
}
