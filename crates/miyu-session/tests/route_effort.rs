//! 思考强度（`docs/blueprint/models.md`「怎么走」第十一条，施工 8-18；8-18（补）去掉会话那一层，只剩配置的默认）：一次
//! 请求照真发的那个模型配置的默认挑一档，没有就不带；换模型以后用新模型自己的；轮换的池里每个成员用自己的；个人设置
//! 压着系统配置，改了下一轮生效；给头看的那一档带着从配置的哪一层来；空闲超时照那一档放大。
//!
//! 两台假服务器，档案是空的、没有目录：档位全照手写的 `reasoning`。

mod support;

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::watch as channel;

use miyu_config::Layer;
use miyu_config::merge::{Layers, Resolved, merge};
use miyu_config::parse::parse;
use miyu_http::testkit::{Piece, Reply, Server};
use miyu_kernel::event::{
    CallResult, ChangeWhy, EffortInUse, EffortSource, ErrorClass, ModelChanged, TransientBody,
};
use miyu_kernel::session::Command;
use miyu_session::{ConfigSource, Handle, Models, Pushed, fixed_with};
use miyu_tool::Catalog;
use support::routing::{called, configs, hellos, items, routes};
use support::{Home, Lines, Opening, ask, say, until_turn_ends, watch};

/// 两家 `a`、`b`，都不带 key。`a` 的 `m` 有 `a_levels` 那几档、默认 `low`；`b` 的 `n` 有 `high`、`max`，没有默认。池 `p`
/// 是两个都有的轮换。`models.chat` 是 `a/m`。
fn config(first: &Server, second: &Server, a_levels: &str) -> String {
    format!(
        "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[providers.a.models.m]\nreasoning = [{a_levels}]\neffort = \"low\"\n\n\
         [providers.b]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[providers.b.models.n]\nreasoning = [\"high\", \"max\"]\n\n\
         [pools.p]\nmodels = [\"a/m\", \"b/n\"]\nstrategy = \"rotate\"\n\n[models]\nchat = \"a/m\"\n",
        first.base_url, second.base_url
    )
}

const A_LEVELS: &str = "\"off\", \"low\", \"high\"";

/// 造一个会话，记着的模型是 `model`。
async fn create(home: &Home, models: &dyn Models, model: &str) -> Handle {
    let lines = Lines {
        model: Some(model.to_string()),
        ..Lines::default()
    };
    home.create_full(models, &Catalog::default(), Opening::default(), lines)
        .await
}

/// 说一句、等这一轮说完，交回这一轮推过来的 `model.changed`。
async fn turn(handle: &Handle, command: &str) -> Vec<ModelChanged> {
    let mut pushes = watch(handle).await;
    ask(handle, command, say("hi")).await.expect("会话在跑");
    let pushed = until_turn_ends(&mut pushes).await;
    pushed
        .iter()
        .filter_map(|pushed| match &**pushed {
            Pushed::Transient(transient) => match &transient.body {
                TransientBody::ModelChanged(changed) => Some((**changed).clone()),
                _ => None,
            },
            Pushed::Events(_) => None,
        })
        .collect()
}

/// 换模型。
async fn switch(handle: &Handle, command: &str, model: &str) {
    let configure = Command::Configure {
        model: model.to_string(),
    };
    ask(handle, command, configure).await.expect("会话在跑");
}

/// 一台服务器收到的每一份请求里的 `reasoning_effort`，照先后；没带的是 `-`。
fn sent(server: &Server) -> Vec<String> {
    server
        .received()
        .iter()
        .map(|received| {
            let body: serde_json::Value =
                serde_json::from_slice(&received.body).expect("请求是 JSON");
            body.get("reasoning_effort")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("-")
                .to_string()
        })
        .collect()
}

fn used(level: &str, from: EffortSource) -> Option<EffortInUse> {
    Some(EffortInUse {
        level: level.to_string(),
        from,
    })
}

#[tokio::test]
async fn a_request_takes_the_configured_default_or_nothing() {
    let (first, second) = (
        Server::start(hellos(12)).await,
        Server::start(hellos(12)).await,
    );
    let mut home = Home::new();
    home.configs = configs(&config(&first, &second, A_LEVELS), &[]);
    let routes = routes(serde_json::json!({}), Duration::from_secs(5));
    let handle = create(&home, &routes, "a/m").await;
    assert_eq!(
        handle.next().effort,
        used("low", EffortSource::System),
        "造好就看得到"
    );
    turn(&handle, "cmd-1").await;
    assert_eq!(
        sent(&first).first().map(String::as_str),
        Some("low"),
        "配置的默认"
    );
    // 换模型以后用新模型自己的：`b/n` 没有默认，什么都不带。
    switch(&handle, "cmd-2", "b/n").await;
    let changed = turn(&handle, "cmd-3").await;
    assert_eq!(changed[0].effort, None);
    assert_eq!(
        sent(&second).last().map(String::as_str),
        Some("-"),
        "什么都不带"
    );
}

#[tokio::test]
async fn each_member_of_a_rotating_pool_uses_its_own_config_default() {
    let (first, second) = (
        Server::start(hellos(8)).await,
        Server::start(hellos(8)).await,
    );
    let text = format!(
        "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[providers.a.models.m]\nreasoning = [\"off\", \"low\", \"high\"]\neffort = \"off\"\n\n\
         [providers.b]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[providers.b.models.n]\nreasoning = [\"high\", \"max\"]\neffort = \"max\"\n\n\
         [pools.p]\nmodels = [\"a/m\", \"b/n\"]\nstrategy = \"rotate\"\n\n[models]\nchat = \"a/m\"\n",
        first.base_url, second.base_url
    );
    let mut home = Home::new();
    home.configs = configs(&text, &[]);
    let routes = routes(serde_json::json!({}), Duration::from_secs(5));
    let handle = create(&home, &routes, "@p").await;
    let changed = turn(&handle, "cmd-1").await;
    assert!(
        changed.iter().all(|changed| changed.effort.is_none()),
        "轮换的池没有单一的模型，不带"
    );
    turn(&handle, "cmd-2").await;
    assert_eq!(handle.next().effort, None);
    let (a, b) = (sent(&first), sent(&second));
    assert!(
        !a.is_empty() && !b.is_empty(),
        "两个成员都发过：{a:?} {b:?}"
    );
    assert!(
        a.iter().all(|level| level == "none"),
        "a/m 配的是 off，没有开关的发 none：{a:?}"
    );
    assert!(b.iter().all(|level| level == "max"), "{b:?}");
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

/// 个人设置压着系统配置：造好时系统配置的 `low` 生效，`from` 是 `system`；改了个人设置下一轮生效，`from` 换成 `personal`
/// （施工 8-18（补），「怎么走」第十一条第 2、4、7 条）。
#[tokio::test]
async fn a_personal_setting_overrides_the_system_default_on_the_next_turn() {
    let (first, second) = (
        Server::start(hellos(12)).await,
        Server::start(hellos(12)).await,
    );
    let system = config(&first, &second, A_LEVELS);
    let (switching, receiving) = channel::channel(two_layers(&system, ""));
    let mut home = Home::new();
    home.configs = receiving;
    let routes = routes(serde_json::json!({}), Duration::from_secs(5));
    let handle = create(&home, &routes, "a/m").await;
    assert_eq!(handle.next().effort, used("low", EffortSource::System));
    let changed = turn(&handle, "cmd-1").await;
    assert_eq!(
        sent(&first).first().map(String::as_str),
        Some("low"),
        "先是系统配置"
    );
    assert!(changed.is_empty(), "造好就看得到的，不用再推一次");
    // 个人设置写了 high：这一轮还没生效。
    let personal = "[providers.a.models.m]\neffort = \"high\"\n";
    switching.send_replace(two_layers(&system, personal));
    assert_eq!(
        handle.next().effort,
        used("low", EffortSource::System),
        "下一个回合开始才生效"
    );
    let changed = turn(&handle, "cmd-2").await;
    assert_eq!(changed[0].effort, used("high", EffortSource::Personal));
    assert_eq!(
        changed[0].why,
        ChangeWhy::Turn,
        "回合开始重新解析完，强度变了也推"
    );
    assert_eq!(handle.next().effort, used("high", EffortSource::Personal));
    assert_eq!(
        sent(&first).last().map(String::as_str),
        Some("high"),
        "个人设置压着系统配置"
    );
}

/// 空闲超时照那一档放大：基数 300 毫秒，服务器 700 毫秒以后才开口。`max` 放大 4 倍，等得到；没写的照基数，超时、再来一次。
#[tokio::test]
async fn the_idle_timeout_grows_with_the_level() {
    let slow = || {
        let mut replies = vec![Reply::stream(vec![Piece::Wait(Duration::from_millis(700))])];
        replies[0].body.extend(hellos(1).remove(0).body);
        replies.extend(hellos(4));
        replies
    };
    for (written, first_ok) in [("effort = \"max\"\n", true), ("", false)] {
        let server = Server::start(slow()).await;
        let text = format!(
            "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[providers.a.models.m]\nreasoning = [\"high\", \"max\"]\n{written}\n[models]\nchat = \"a/m\"\n",
            server.base_url
        );
        let mut home = Home::new();
        home.configs = configs(&text, &[]);
        let routes = routes(serde_json::json!({}), Duration::from_millis(300));
        let handle = create(&home, &routes, "a/m").await;
        turn(&handle, "cmd-1").await;
        let first = called(&home, &handle)
            .into_iter()
            .find(|call| call.purpose.is_none())
            .expect("发过");
        match first_ok {
            true => assert_eq!(first.result, CallResult::Ok, "放大了，等得到"),
            false => assert_eq!(
                first.error.map(|error| error.class),
                Some(ErrorClass::Retryable),
                "照基数，超时"
            ),
        }
    }
}
