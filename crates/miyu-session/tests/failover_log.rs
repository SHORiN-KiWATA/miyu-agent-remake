//! 换端点的两行运行日志（`docs/blueprint/models.md`「出错」运行时那张表、`log.md`，施工 8-9）：记冷却的
//! `endpoint cooling`，换到池里别的成员的 `failover`（从哪个到哪个），都带会话编号；key 的值不进日志。施工 8-25 起一家一个
//! key，换端点只在池的成员之间。
//!
//! 只有这一个测试，自己一个进程：`tracing` 的调用点第一次被碰到时记下谁在听，别的测试同时碰到，这里装的订阅者可能漏听。

mod support;

use std::time::Duration;

use miyu_config::secret::Reference;
use miyu_http::testkit::{Reply, Server};
use miyu_log::{LevelFilter, Memory};
use support::routing::{configs, hellos, routes};
use support::{Home, ask, say, until_turn_ends, watch};

#[tokio::test]
async fn cooling_and_failover_are_logged_without_the_key() {
    let memory = Memory::new();
    let _listening = tracing::subscriber::set_default(miyu_log::subscriber(
        memory.clone(),
        LevelFilter::INFO,
        None,
    ));
    let mut replies = vec![Reply::error(429, &[], "{}")];
    replies.extend(hellos(4));
    let server = Server::start(replies).await;
    let source = format!(
        "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"{url}\"\nkey = {{ env = \"K1\" }}\n\n\
         [providers.b]\ndriver = \"openai-chat\"\nbase_url = \"{url}\"\nkey = {{ env = \"K2\" }}\n\n\
         [pools.p]\nmodels = [\"a/m\", \"b/m\"]\nstrategy = \"pin\"\n\n[models]\nchat = \"@p\"\n",
        url = server.base_url
    );
    let mut home = Home::new();
    home.configs = configs(
        &source,
        &[
            (Reference::Env("K1".to_string()), "sk-one"),
            (Reference::Env("K2".to_string()), "sk-two"),
        ],
    );
    let routes = routes(serde_json::json!({}), Duration::from_secs(60));
    let handle = home.create(&routes).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;

    let lines = memory.lines();
    let session = handle.id().as_str();
    for wanted in [
        format!(
            " INFO  session  {session} endpoint cooling provider=a model=m class=rate_limited for_ms=30000 failures=1"
        ),
        format!(" INFO  session  {session} failover from=a/m to=b/m class=rate_limited"),
    ] {
        assert!(
            lines.iter().any(|line| line.contains(&wanted)),
            "「{wanted}」应该在 {lines:#?} 里"
        );
    }
    assert!(
        !lines
            .iter()
            .any(|line| line.contains("sk-one") || line.contains("sk-two")),
        "key 的值不进日志：{lines:#?}"
    );
}
