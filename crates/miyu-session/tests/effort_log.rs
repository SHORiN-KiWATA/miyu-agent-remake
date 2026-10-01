//! 会话记的思考强度不在档位里了的那一行运行日志（`docs/blueprint/models.md`「出错」运行时那张表，施工 8-18）：
//! `WARN effort not available model=… level=…`，带会话编号。
//!
//! 只有这一个测试，自己一个进程：`tracing` 的调用点第一次被碰到时记下谁在听，别的测试同时碰到，这里装的订阅者可能漏听。

mod support;

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::watch;

use miyu_http::testkit::Server;
use miyu_kernel::event::Effort;
use miyu_kernel::session::Command;
use miyu_log::{LevelFilter, Memory};
use miyu_session::ConfigSource;
use miyu_tool::Catalog;
use support::routing::{configs, hellos, routes, turn};
use support::{Home, Lines, Opening, ask};

#[tokio::test]
async fn a_cell_no_longer_offered_is_logged() {
    let memory = Memory::new();
    let _listening = tracing::subscriber::set_default(miyu_log::subscriber(
        memory.clone(),
        LevelFilter::INFO,
        None,
    ));
    let server = Server::start(hellos(4)).await;
    let config = |levels: &str| -> Arc<dyn ConfigSource> {
        let text = format!(
            "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[providers.a.models.m]\nreasoning = [{levels}]\n\n[models]\nchat = \"a/m\"\n",
            server.base_url
        );
        Arc::clone(&*configs(&text, &[]).borrow())
    };
    let (switch, receiving) = watch::channel(config("\"low\", \"high\""));
    let mut home = Home::new();
    home.configs = receiving;
    let routes = routes(serde_json::json!({}), Duration::from_secs(5));
    let lines = Lines {
        model: Some("a/m".to_string()),
        ..Lines::default()
    };
    let handle = home
        .create_full(&routes, &Catalog::default(), Opening::default(), lines)
        .await;
    let configure = Command::Configure {
        model: None,
        effort: Some(Effort {
            model: "a/m".to_string(),
            level: Some("high".to_string()),
        }),
    };
    ask(&handle, "cmd-1", configure).await.expect("会话在跑");
    switch.send_replace(config("\"low\""));
    turn(&handle, "cmd-2").await;

    let lines = memory.lines();
    let wanted = format!(
        " WARN  session  {} effort not available model=a/m level=high",
        handle.id().as_str()
    );
    assert!(
        lines.iter().any(|line| line.contains(&wanted)),
        "「{wanted}」应该在 {lines:#?} 里"
    );
}
