//! 温度（`docs/blueprint/models.md`「怎么走」第十四条，施工 8-22）：一次请求照真发的那个模型配置的默认带，没有就不带；
//! 换模型以后用新模型自己的；轮换的池里每个成员用自己的；个人设置压着系统配置，改了下一轮生效。
//!
//! 两台假服务器，档案是空的、没有目录：没有目录说不收，温度都当能调。

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::watch as channel;

use crate::support::routing::{configs, hellos, items, routes};
use crate::support::{Home, Lines, Opening, ask, say, until_turn_ends, watch};
use miyu_config::Layer;
use miyu_config::merge::{Layers, Resolved, merge};
use miyu_config::parse::parse;
use miyu_http::testkit::Server;
use miyu_kernel::session::Command;
use miyu_session::{ConfigSource, Handle, Models, fixed_with};
use miyu_tool::Catalog;
use serde_json::Value;

/// 两家 `a`、`b`，都不带 key。`a` 的 `m` 默认 0.7；`b` 的 `n` 没写。池 `p` 是两个都有的轮换。`models.chat` 是 `a/m`。
fn config(first: &Server, second: &Server, extra: &str) -> String {
    format!(
        "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[providers.a.models.m]\ntemperature = 0.7\n\n\
         [providers.b]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[providers.b.models.n]\n{extra}\n\
         [pools.p]\nmodels = [\"a/m\", \"b/n\"]\nstrategy = \"rotate\"\n\n[models]\nchat = \"a/m\"\n",
        first.base_url, second.base_url
    )
}

/// 造一个会话，记着的模型是 `model`。
async fn create(home: &Home, models: &dyn Models, model: &str) -> Handle {
    let lines = Lines {
        model: Some(model.to_string()),
        ..Lines::default()
    };
    home.create_full(models, &Catalog::default(), Opening::default(), lines)
        .await
}

/// 说一句、等这一轮说完。
async fn turn(handle: &Handle, command: &str) {
    let mut pushes = watch(handle).await;
    ask(handle, command, say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
}

/// 一台服务器收到的每一份请求里的 `temperature`，照先后；没带的是 `null`。
fn sent(server: &Server) -> Vec<Value> {
    server
        .received()
        .iter()
        .map(|received| {
            let body: Value = serde_json::from_slice(&received.body).expect("请求是 JSON");
            body.get("temperature").cloned().unwrap_or(Value::Null)
        })
        .collect()
}

#[tokio::test]
async fn a_request_takes_the_configured_default_or_nothing() {
    let (first, second) = (
        Server::start(hellos(12)).await,
        Server::start(hellos(12)).await,
    );
    let mut home = Home::new();
    home.configs = configs(&config(&first, &second, ""), &[]);
    let routes = routes(serde_json::json!({}), Duration::from_secs(60));
    let handle = create(&home, &routes, "a/m").await;
    turn(&handle, "cmd-1").await;
    let a = sent(&first);
    assert!(!a.is_empty(), "发过");
    assert!(
        a.iter().all(|value| *value == 0.7),
        "主请求、辅助请求都带配置的默认：{a:?}"
    );
    // 换模型以后用新模型自己的：`b/n` 没写，什么都不带。
    let configure = Command::Configure {
        model: "b/n".to_string(),
    };
    ask(&handle, "cmd-2", configure).await.expect("会话在跑");
    turn(&handle, "cmd-3").await;
    let b = sent(&second);
    assert!(
        !b.is_empty() && b.iter().all(Value::is_null),
        "什么都不带：{b:?}"
    );
}

#[tokio::test]
async fn each_member_of_a_rotating_pool_uses_its_own_config_default() {
    let (first, second) = (
        Server::start(hellos(8)).await,
        Server::start(hellos(8)).await,
    );
    let mut home = Home::new();
    home.configs = configs(&config(&first, &second, "temperature = 1.4\n"), &[]);
    let routes = routes(serde_json::json!({}), Duration::from_secs(60));
    let handle = create(&home, &routes, "@p").await;
    turn(&handle, "cmd-1").await;
    turn(&handle, "cmd-2").await;
    let (a, b) = (sent(&first), sent(&second));
    assert!(
        !a.is_empty() && !b.is_empty(),
        "两个成员都发过：{a:?} {b:?}"
    );
    assert!(a.iter().all(|value| *value == 0.7), "{a:?}");
    assert!(b.iter().all(|value| *value == 1.4), "{b:?}");
}

/// 系统配置 `system`、个人设置 `personal` 两层合出来的最终值，当成一份不变的配置源。
fn two_layers(system: &str, personal: &str) -> Arc<dyn ConfigSource> {
    let item_list = items();
    let system = parse(&item_list, Layer::System, system).expect("写法对");
    let personal = parse(&item_list, Layer::Personal, personal).expect("写法对");
    let layers = Layers {
        system: Some(&system),
        personal: Some(&personal),
        ..Layers::default()
    };
    let resolved: Resolved = merge(&item_list, &layers, &|_| None);
    Arc::clone(&*fixed_with(resolved, Vec::new()).borrow())
}

/// 个人设置压着系统配置：改了个人设置下一轮生效（`next_turn`）。
#[tokio::test]
async fn a_personal_setting_overrides_the_system_default_on_the_next_turn() {
    let (first, second) = (
        Server::start(hellos(12)).await,
        Server::start(hellos(12)).await,
    );
    let system = config(&first, &second, "");
    let (switching, receiving) = channel::channel(two_layers(&system, ""));
    let mut home = Home::new();
    home.configs = receiving;
    let routes = routes(serde_json::json!({}), Duration::from_secs(60));
    let handle = create(&home, &routes, "a/m").await;
    turn(&handle, "cmd-1").await;
    assert_eq!(
        sent(&first).first(),
        Some(&Value::from(0.7)),
        "先是系统配置"
    );
    switching.send_replace(two_layers(
        &system,
        "[providers.a.models.m]\ntemperature = 0\n",
    ));
    turn(&handle, "cmd-2").await;
    assert_eq!(
        sent(&first).last(),
        Some(&Value::from(0)),
        "个人设置压着系统配置，0 也是写了的"
    );
}
