//! 运行日志（`docs/designs/28-运行日志.md`，施工 3-7 上）：发一次请求，`DEBUG` 记发出去了、怎么收场的
//! 两行；key、请求体和回复里的字、地址的路径和参数、出错的原话，一个字都不记。
//!
//! 只有这一个测试，自己一个进程：`tracing` 的调用点第一次被碰到时记下谁在听，别的测试同时碰到，
//! 这里装的订阅者可能漏听。

mod support;

use std::future::Future;
use std::time::Duration;

use miyu_http::testkit::{Piece, Reply, Server};
use miyu_http::{Attempt, Endpoint, Outcome, Proxy, client, send};
use miyu_log::{LevelFilter, Memory};
use support::{BODY, driver, sample};

/// 发一次，交回收场。路径里带着 key，像把 key 放在地址里的供应商。
async fn run(endpoint: &Endpoint, cancel: impl Future<Output = ()> + Send) -> Outcome {
    let client = client(Proxy::Off).expect("造得出客户端");
    let driver = driver();
    send(
        Attempt {
            client: &client,
            endpoint,
            driver: &driver,
            body: BODY,
            path: "/chat/completions?key=sk-test",
            idle: Duration::from_secs(5),
        },
        cancel,
        |_| {},
    )
    .await
}

/// 一行去掉时刻，用时换成 `_`：这两样每次不一样。
fn shape(line: &str) -> String {
    let rest = line.splitn(3, ' ').nth(2).unwrap_or_default();
    match rest.split_once(" took_ms=") {
        Some((head, took)) if !took.is_empty() && took.bytes().all(|b| b.is_ascii_digit()) => {
            format!("{head} took_ms=_")
        }
        _ => rest.to_string(),
    }
}

#[tokio::test]
async fn the_log_says_what_happened_and_nothing_of_what_was_said() {
    let memory = Memory::new();
    let _listening =
        tracing::subscriber::set_default(miyu_log::subscriber(memory.clone(), LevelFilter::DEBUG));

    // 说完的一次：回复里也有「你好」。
    let server = Server::start(vec![Reply::stream(vec![Piece::Bytes(sample(
        "openai-text",
    ))])])
    .await;
    let endpoint = Endpoint::new(&server.base_url, "sk-test");
    let outcome = run(&endpoint, std::future::pending()).await;
    assert!(
        matches!(outcome, Outcome::Ended { error: None, .. }),
        "{outcome:?}"
    );

    // 限速的一次：出错的原话里回显了 key 和请求里的字。
    let server = Server::start(vec![Reply::error(
        429,
        &[("Retry-After", "7")],
        r#"{"error":{"message":"Rate limit reached for sk-test: 你好"}}"#,
    )])
    .await;
    let endpoint = Endpoint::new(&server.base_url, "sk-test");
    let outcome = run(&endpoint, std::future::pending()).await;
    assert!(
        matches!(outcome, Outcome::Ended { error: Some(_), .. }),
        "{outcome:?}"
    );

    // 连不上的一次：没有状态码。先占一个端口再放掉，上面没人听。
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("绑得上");
    let port = listener.local_addr().expect("有地址").port();
    drop(listener);
    let endpoint = Endpoint::new(format!("http://127.0.0.1:{port}/v1"), "sk-test");
    let outcome = run(&endpoint, std::future::pending()).await;
    assert!(
        matches!(outcome, Outcome::Ended { error: Some(_), .. }),
        "{outcome:?}"
    );

    // 地址读不出主机名的一次：主机名写 `?`，发不出去。
    let endpoint = Endpoint::new("not a url", "sk-test");
    let outcome = run(&endpoint, std::future::pending()).await;
    assert!(
        matches!(outcome, Outcome::Ended { error: Some(_), .. }),
        "{outcome:?}"
    );

    // 叫停的一次：回完了头、停住了再叫停。
    let server = Server::start(vec![Reply::stream(vec![Piece::Stall])]).await;
    let endpoint = Endpoint::new(&server.base_url, "sk-test");
    let outcome = run(&endpoint, server.wait_stalled(1)).await;
    assert_eq!(outcome, Outcome::Cancelled);

    let lines = memory.lines();
    let sent = format!("DEBUG http     sent host=127.0.0.1 bytes={}", BODY.len());
    let nowhere = format!("DEBUG http     sent host=? bytes={}", BODY.len());
    assert_eq!(
        lines.iter().map(|line| shape(line)).collect::<Vec<_>>(),
        [
            sent.as_str(),
            "DEBUG http     ended host=127.0.0.1 status=200 took_ms=_",
            sent.as_str(),
            "DEBUG http     failed host=127.0.0.1 status=429 class=rate_limited \
             retry_after_ms=7000 took_ms=_",
            sent.as_str(),
            "DEBUG http     failed host=127.0.0.1 class=retryable took_ms=_",
            nowhere.as_str(),
            "DEBUG http     failed host=? class=retryable took_ms=_",
            sent.as_str(),
            "DEBUG http     cancelled host=127.0.0.1 took_ms=_",
        ],
        "{lines:#?}"
    );
    // 一个字都不记：key、请求体和回复里的字、地址的路径和参数、出错的原话。
    for line in &lines {
        for said in [
            "sk-test",
            "你好",
            "/v1",
            "completions",
            "key=",
            "Rate limit",
        ] {
            assert!(!line.contains(said), "日志里有「{said}」：{line}");
        }
    }
}
