//! 用出来的窗口（施工 8-7，`docs/blueprint/models.md`「怎么走」第二条第 9 条）：请求报上下文超长、说了上限、比手头的
//! 窗口小的，记进 `state/models/learned.json`、记一行 `INFO learned window`，新造的会话用上；比手头的大的不记。
//!
//! 只有这一个测试，自己一个进程：要接住阻塞线程里发的运行日志（见 `blocking_log.rs`）。

mod support;

use std::sync::Arc;
use std::time::Duration;

use miyu_http::testkit::{Piece, Reply, Server};
use miyu_log::{LevelFilter, Memory};
use miyu_models::matching::Vendors;
use miyu_models::observed::Learned;
use miyu_models::profile::Profiles;
use miyu_session::{ModelData, Observed, read_observed};
use support::routing::{configs, routes_with};
use support::{Home, ask, say, until_turn_ends, watch};

/// 超长的 400：上限 `limit`。
fn too_long(limit: u64) -> Reply {
    let body = format!(
        r#"{{"error":{{"message":"This model's maximum context length is {limit} tokens. However, you requested {} tokens.","code":"context_length_exceeded"}}}}"#,
        limit + 5000
    );
    Reply::error(400, &[], &body)
}

/// 说「你好！」。
fn hello() -> Reply {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/designs/samples/drivers/openai-chat/streams/openai-text.sse");
    Reply::stream(vec![Piece::Bytes(
        std::fs::read(&path).expect("样本读得到"),
    )])
}

/// 一家 `dev` 在 `base_url`，模型 `m`，手写的窗口另写 `window` 那一行。
fn config(base_url: &str, window: &str) -> String {
    format!(
        "[providers.dev]\ndriver = \"openai-chat\"\nbase_url = \"{base_url}\"\n\n[providers.dev.models.m]\n{window}\n[models]\nchat = \"dev/m\"\n"
    )
}

/// 说一句、等这一轮了结（可能以出错了结），交回新造的会话。
async fn overflow(home: &mut Home, data: &Arc<ModelData>, limit: u64, window: &str) {
    let mut replies = vec![too_long(limit)];
    replies.extend((0..6).map(|_| hello()));
    let server = Server::start(replies).await;
    home.configs = configs(&config(&server.base_url, window), &[]);
    let routes = routes_with(Arc::clone(data), Duration::from_secs(5));
    let handle = home.create(&routes).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    tokio::time::timeout(Duration::from_secs(20), until_turn_ends(&mut pushes))
        .await
        .expect("这一轮了结");
}

#[tokio::test]
async fn a_stated_limit_is_learned_and_used_by_the_next_session() {
    let memory = Memory::new();
    // 记下用出来的那一行在写盘的阻塞线程里发：装成全局的。
    tracing::subscriber::set_global_default(miyu_log::subscriber(
        memory.clone(),
        LevelFilter::INFO,
        None,
    ))
    .expect("这个测试程序只装这一次");
    let mut home = Home::new();
    let dir = home.root.state().join("models");
    let data = Arc::new(ModelData::new(
        Profiles::default(),
        Vendors::default(),
        Some(dir.clone()),
    ));
    data.loaded(None, Observed::default());
    overflow(&mut home, &data, 65_536, "").await;
    let learned = Learned::parse(&std::fs::read_to_string(dir.join("learned.json")).expect("写了"))
        .expect("读得进");
    assert_eq!(
        learned.window("dev", "m").map(|stamped| stamped.value),
        Some(65_536)
    );
    assert_eq!(
        read_observed(&dir).learned,
        learned,
        "核心下次起来读回同一份"
    );
    let lines = memory.lines();
    assert!(
        lines
            .iter()
            .any(|line| line.contains("learned window provider=dev model=m window=65536")),
        "{lines:#?}"
    );
    // 新造的会话用上。
    let routes = routes_with(Arc::clone(&data), Duration::from_secs(5));
    assert_eq!(home.create(&routes).await.limits().window, Some(65_536));
    // 再报一个更大的上限：不记。
    overflow(&mut home, &data, 100_000, "").await;
    assert_eq!(
        data.with(|knowledge| knowledge
            .learned
            .window("dev", "m")
            .map(|stamped| stamped.value)),
        Some(65_536)
    );
    // 手写的窗口比报的小：不记；手写的盖过用出来的。
    overflow(&mut home, &data, 60_000, "window = 50000").await;
    let routes = routes_with(Arc::clone(&data), Duration::from_secs(5));
    assert_eq!(
        data.with(|knowledge| knowledge
            .learned
            .window("dev", "m")
            .map(|stamped| stamped.value)),
        Some(65_536)
    );
    assert_eq!(home.create(&routes).await.limits().window, Some(50_000));
}
