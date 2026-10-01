//! 替看不了图的模型看图的运行日志（`docs/blueprint/models.md`「出错」、「怎么走」第十三条第 4、7 条，施工 8-17）：没成的 actor
//! 记一行 `image not described`，带会话编号、哪一张图、为什么；成了的不另记，一次性入口那一行 `model call` 带着会话编号。
//!
//! 只有这一个测试，自己一个进程：`tracing` 的调用点第一次被碰到时记下谁在听，别的测试同时碰到，这里装的订阅者可能漏听
//! （和 `log.rs` 一样）。

mod support;

use std::time::Duration;

use miyu_http::testkit::Server;
use miyu_kernel::block::{Block, Image};
use miyu_kernel::id::{ContentHash, MediaType};
use miyu_kernel::session::Command;
use miyu_log::{LevelFilter, Memory};
use miyu_session::Handle;
use miyu_store::blob::Blobs;
use support::routing::{configs, hellos, routes};
use support::{Home, alice_account, ask, stop, until_turn_ends, watch};

/// 只发一张图，等这一轮说完。
async fn show(handle: &Handle, blob: &ContentHash) {
    let mut pushes = watch(handle).await;
    let send = Command::Send {
        blocks: vec![Block::Image(Image {
            blob: blob.clone(),
            name: None,
            media_type: MediaType::parse("image/png").expect("媒体类型合写法"),
            width: 1,
            height: 1,
        })],
        urgent: false,
    };
    ask(handle, "cmd-1", send).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
}

#[tokio::test]
async fn a_failed_description_writes_a_line_and_a_good_one_does_not() {
    let memory = Memory::new();
    let _listening = tracing::subscriber::set_default(miyu_log::subscriber(
        memory.clone(),
        LevelFilter::INFO,
        None,
    ));
    let (first, second) = (
        Server::start(hellos(8)).await,
        Server::start(hellos(1)).await,
    );
    let source = |extra: &str| {
        format!(
            "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[providers.b]\ndriver = \"openai-chat\"\n\
             base_url = \"{}\"\n\n[providers.b.models.v]\ninputs = [\"text\", \"image\"]\n\n[models]\nchat = \"a/m\"\n{extra}",
            first.base_url, second.base_url
        )
    };
    let routes = routes(serde_json::json!({}), Duration::from_secs(5));
    let mut home = Home::new();
    let blob = Blobs::new(home.root.blobs(&alice_account()))
        .put(b"miyu")
        .expect("存得进去");
    home.configs = configs(&source(""), &[]);
    let without = home.create(&routes).await;
    show(&without, &blob).await;
    stop(&without).await;
    home.configs = configs(&source("vision = \"b/v\"\n"), &[]);
    let with = home.create(&routes).await;
    show(&with, &blob).await;
    stop(&with).await;
    let lines = memory.lines();
    let s = without.id().as_str();
    let missing = format!(
        "INFO  session  {s} image not described blob={blob} why=\"no vision model configured: set models.vision\""
    );
    assert!(
        lines.iter().any(|line| line.ends_with(&missing)),
        "{lines:#?}"
    );
    let t = with.id().as_str();
    assert!(
        lines.iter().any(|line| line.contains(&format!(
            "session  {t} model call purpose=vision provider=b model=v"
        ))),
        "一次性入口那一行带着会话编号：{lines:#?}"
    );
    assert!(
        lines
            .iter()
            .all(|line| !(line.contains(t) && line.contains("image not described"))),
        "成了的不另记"
    );
}
