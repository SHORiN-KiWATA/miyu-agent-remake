//! 发一次请求（`docs/designs/05-内核接口.md` 第七节「HTTP 执行器」）：照剧本回的假服务器，查发出去的、
//! 读回来的、空闲超时、打断、连不上、说到一半断开。

mod support;

use std::path::PathBuf;
use std::time::Duration;

use miyu_drivers::openai_chat::{Compat, Decoder};
use miyu_drivers::{DriverTextSources, DriverTexts, OpenAiChat};
use miyu_http::{Attempt, Endpoint, Outcome, Progress, Proxy, client, send};
use miyu_kernel::accumulate::Delta;
use miyu_kernel::event::ErrorClass;
use miyu_kernel::id::ContentHash;
use support::{Piece, Reply, Server};

/// 一份请求字节：内容无所谓，查的是一字不差地发出去。
const BODY: &[u8] =
    r#"{"model":"deepseek-v4","messages":[{"role":"user","content":"你好"}],"stream":true}"#
        .as_bytes();

fn driver() -> OpenAiChat {
    let texts = DriverTexts::new(DriverTextSources {
        image_omitted: "image\n",
        file_omitted: "file {name} {media_type}\n",
        no_output: "nothing\n",
        tool_attachments: "attachments\n",
        tool_attachments_only: "only\n",
    })
    .expect("占位用得了");
    OpenAiChat::new(Compat::default(), texts)
}

/// 驱动的流的样本。
fn sample(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/designs/samples/drivers/openai-chat/streams")
        .join(format!("{name}.sse"));
    std::fs::read(&path).unwrap_or_else(|e| panic!("读不了 {}：{e}", path.display()))
}

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
    let cancel = tokio::time::sleep(Duration::from_millis(100));
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
    let endpoint = Endpoint::new(format!("http://127.0.0.1:{port}/v1"), "sk-test");
    let (progress, outcome) = run(&endpoint, Duration::from_secs(5), never()).await;
    assert!(matches!(progress.first(), Some(Progress::Sent { .. })));
    assert_eq!(class(&outcome), Some(ErrorClass::Retryable));
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
