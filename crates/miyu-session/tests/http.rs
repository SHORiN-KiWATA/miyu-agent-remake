//! 经驱动和 HTTP 请求模型（`docs/construction/3-7-会话actor（下）.md` 验收第 2 条）：本机的假服务器照样本
//! 的流回；限速了等够再请求；打断了断开连接；编码要的 blob 取不出来，出错、不发。

mod support;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use miyu_drivers::Inputs;
use miyu_http::testkit::{Piece, Reply, Server};
use miyu_kernel::block::{Block, Image, Text};
use miyu_kernel::event::{Body, CallResult, ErrorClass, Event, ModelCalled, Usage};
use miyu_kernel::id::{ContentHash, MediaType, ModelName, ProviderId};
use miyu_kernel::session::{Command, Outcome, Queued};
use miyu_session::Routes;
use miyu_store::blob::Blobs;
use support::{Home, alice_account, ask, say, until_turn_ends, watch, within};

/// 发给假服务器的路由（施工 8-6 起照配置）：deepseek 的 deepseek-v4，OpenAI 兼容的写法，空闲超时五秒；`inputs` 照档案，
/// 接着写的照 `continues` 用 DeepSeek 的那一套开关。会话的配置换成指到这台服务器的那一份。
fn models(home: &mut Home, server: &Server, inputs: Inputs, continues: bool) -> Routes {
    let mut profile = serde_json::json!({});
    if inputs.images {
        profile["inputs"] = serde_json::json!(["text", "image"]);
    }
    if continues {
        profile["compat"] = serde_json::json!({
            "reasoning": {"replay": "reasoning_content", "always": true},
            "continuation": {"field": "prefix", "path": "/beta/chat/completions"}
        });
    }
    let (routes, configs) = support::routing::served(&server.base_url, profile);
    home.configs = configs;
    routes
}

/// 驱动的流的样本：说「你好！」，用量 80 + 1920（命中）+ 3。
fn said_hello() -> Reply {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/designs/samples/drivers/openai-chat/streams/openai-text.sse");
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("读不了 {}：{e}", path.display()));
    Reply::stream(vec![Piece::Bytes(bytes)])
}

/// 日志里的 `model.called`，照先后。
fn called(log: &[Event]) -> Vec<&ModelCalled> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::ModelCalled(called) => Some(called),
            _ => None,
        })
        .collect()
}

/// 日志里她的最后一条回复的内容块。
fn replied(log: &[Event]) -> Vec<Block> {
    log.iter()
        .rev()
        .find_map(|event| match &event.body {
            Body::MessageAssistant(reply) => Some(reply.blocks.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

fn hello() -> Vec<Block> {
    vec![Block::Text(Text {
        text: "你好！".to_string(),
    })]
}

#[tokio::test]
async fn a_reply_comes_back_from_the_server() {
    let mut home = Home::new();
    let server = Server::start(vec![said_hello()]).await;
    let routes = models(&mut home, &server, Inputs::default(), false);
    let handle = home.create(&routes).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;

    let log = home.log(handle.id());
    assert_eq!(replied(&log), hello());
    let received = server.received();
    assert_eq!(received.len(), 1);
    assert_eq!(received[0].path, "/v1/chat/completions");
    let calls = called(&log);
    assert_eq!(calls.len(), 1);
    // 记下的请求哈希，就是发出去的那串字节的哈希；用量是流里报的。
    assert_eq!(calls[0].request, Some(ContentHash::of(&received[0].body)));
    assert_eq!(
        calls[0].usage,
        Some(Usage {
            uncached: 80,
            cache_read: 1920,
            cache_write: 0,
            output: 3,
        })
    );
    assert_eq!(
        calls[0].endpoint.as_ref().map(ProviderId::as_str),
        Some("deepseek")
    );
    assert_eq!(
        calls[0].model.as_ref().map(ModelName::as_str),
        Some("deepseek-v4")
    );
}

#[tokio::test]
async fn a_rate_limit_waits_as_long_as_the_server_says() {
    let mut home = Home::new();
    let server = Server::start(vec![
        Reply::error(
            429,
            &[("Retry-After", "2")],
            r#"{"error":{"message":"Rate limit reached"}}"#,
        ),
        said_hello(),
    ])
    .await;
    let routes = models(&mut home, &server, Inputs::default(), false);
    let handle = home.create(&routes).await;
    let mut pushes = watch(&handle).await;
    let started = Instant::now();
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    // 供应商说等 2 秒：比内核自己第一次重试等的 1 秒长，等的是供应商说的。
    assert!(
        started.elapsed() >= Duration::from_secs(2),
        "供应商说等 2 秒：{:?}",
        started.elapsed()
    );
    let received = server.received();
    assert_eq!(received.len(), 2, "等够了又请求了一次");
    assert_eq!(
        received[0].body, received[1].body,
        "什么都没收到的，原样重发"
    );
    let log = home.log(handle.id());
    assert_eq!(replied(&log), hello());
    let calls = called(&log);
    assert_eq!(calls.len(), 2);
    assert_eq!(
        calls[0].error.as_ref().map(|error| error.class.clone()),
        Some(ErrorClass::RateLimited)
    );
    // HTTP 状态码一路带进日志（施工 3-5 三补）。
    assert_eq!(
        calls[0].error.as_ref().and_then(|error| error.status),
        Some(429)
    );
}

#[tokio::test]
async fn interrupting_drops_the_connection() {
    let mut home = Home::new();
    let mut server = Server::start(vec![Reply::stream(vec![Piece::Stall])]).await;
    let routes = models(&mut home, &server, Inputs::default(), false);
    let handle = home.create(&routes).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    // 假服务器回完了头、停住了：请求在读流。
    within("假服务器停住", server.wait_stalled(1)).await;
    let outcome = ask(
        &handle,
        "cmd-2",
        Command::Interrupt {
            queued: Queued::Return,
        },
    )
    .await
    .expect("会话在跑");
    assert!(matches!(outcome, Outcome::Accepted { .. }), "{outcome:?}");
    until_turn_ends(&mut pushes).await;
    // 叫停了，HTTP 丢掉连接，假服务器看到断开。
    server.wait_closed(1).await;
}

#[tokio::test]
async fn a_missing_blob_fails_without_sending() {
    let mut home = Home::new();
    let server = Server::start(Vec::new()).await;
    let images = Inputs {
        images: true,
        pdf: false,
    };
    let routes = models(&mut home, &server, images, false);
    let handle = home.create(&routes).await;
    let mut pushes = watch(&handle).await;
    // 一张图，它的 blob 从没存过。
    let missing = ContentHash::of(b"never stored");
    let blocks = vec![
        Block::Text(Text {
            text: "看这张图".to_string(),
        }),
        Block::Image(Image {
            blob: missing.clone(),
            name: None,
            media_type: MediaType::parse("image/png").expect("媒体类型合写法"),
            width: 1,
            height: 1,
        }),
    ];
    ask(
        &handle,
        "cmd-1",
        Command::Send {
            blocks,
            urgent: false,
        },
    )
    .await
    .expect("会话在跑");
    until_turn_ends(&mut pushes).await;

    let log = home.log(handle.id());
    let calls = called(&log);
    assert_eq!(calls.len(), 1, "分类是其他，重试也没用：只请求了一次");
    assert_eq!(calls[0].result, CallResult::Error);
    let error = calls[0].error.as_ref().expect("出错了");
    assert_eq!(error.class, ErrorClass::Unclassified);
    assert!(error.message.contains(missing.hex()), "{}", error.message);
    assert!(server.received().is_empty(), "没发出去");
}

/// 样本的流在第 `n` 条事件之后断开：前面的照发，后面的不发。样本里事件之间是 `\r\n\r\n`（故意的 CRLF）。
fn cut_after(n: usize) -> Reply {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/designs/samples/drivers/openai-chat/streams/openai-text.sse");
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("读不了 {}：{e}", path.display()));
    let mut end = 0;
    for _ in 0..n {
        end += bytes[end..]
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .expect("样本里有这么多条")
            + 4;
    }
    Reply::stream(vec![Piece::Bytes(bytes[..end].to_vec()), Piece::Drop])
}

#[tokio::test]
async fn a_reply_cut_off_goes_on_through_the_continuation_path() {
    let mut home = Home::new();
    // 说到「你好」就断了；再请求时她接着说完。
    let server = Server::start(vec![cut_after(2), said_hello()]).await;
    let deepseek = models(&mut home, &server, Inputs::default(), true);
    let handle = home.create(&deepseek).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let received = server.received();
    assert_eq!(received.len(), 2);
    assert_eq!(received[0].path, "/v1/chat/completions");
    // 会接着写的供应商：半截的回复带着接着写，发到编码交回的那条路径（施工 3-5 再补）。
    assert_eq!(received[1].path, "/v1/beta/chat/completions");
}

#[tokio::test]
async fn a_stalled_reply_times_out_by_the_idle_limit() {
    let mut home = Home::new();
    // 回完头就停住：200 毫秒没收到新的字节，算断了，重试一次说完。
    let server = Server::start(vec![Reply::stream(vec![Piece::Stall]), said_hello()]).await;
    let mut quick = models(&mut home, &server, Inputs::default(), false);
    quick.idle = Duration::from_millis(200);
    let handle = home.create(&quick).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let log = home.log(handle.id());
    let calls = called(&log);
    assert_eq!(calls.len(), 2);
    let error = calls[0].error.as_ref().expect("第一次断了");
    assert_eq!(error.class, ErrorClass::Retryable);
    assert!(error.message.contains("空闲超时"), "{}", error.message);
    assert_eq!(replied(&log), hello());
}

#[tokio::test]
async fn an_image_goes_out_as_its_bytes() {
    let mut home = Home::new();
    let server = Server::start(vec![said_hello()]).await;
    let images = Inputs {
        images: true,
        pdf: false,
    };
    let routes = models(&mut home, &server, images, false);
    let handle = home.create(&routes).await;
    // 属主的 blob 里存着这张「图」：四个字节 miyu，base64 是 bWl5dQ==。
    let blob = Blobs::new(home.root.blobs(&alice_account()))
        .put(b"miyu")
        .expect("存得进去");
    let mut pushes = watch(&handle).await;
    let blocks = vec![Block::Image(Image {
        blob,
        name: None,
        media_type: MediaType::parse("image/png").expect("媒体类型合写法"),
        width: 1,
        height: 1,
    })];
    ask(
        &handle,
        "cmd-1",
        Command::Send {
            blocks,
            urgent: false,
        },
    )
    .await
    .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let received = server.received();
    assert_eq!(received.len(), 1);
    let body = String::from_utf8_lossy(&received[0].body);
    assert!(
        body.contains("data:image/png;base64,bWl5dQ=="),
        "图照它的字节发出去了：{body}"
    );
}
