//! 思考强度（`docs/blueprint/models.md`「怎么走」第十一条，施工 8-18）：一次请求照真发的那个模型挑一档：会话记的、配置的、
//! 都没有就不带；换模型以后用新模型自己的；轮换的池里每个成员用自己的；会话记的不在档位里了照配置的；改了强度下一轮推
//! `model.changed`，给头看的那一档跟着换；载入的照日志拼的；空闲超时照那一档放大。
//!
//! 两台假服务器，档案是空的、没有目录：档位全照手写的 `reasoning`。

mod support;

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::watch as channel;

use miyu_http::testkit::{Piece, Reply, Server};
use miyu_kernel::event::{
    CallResult, ChangeWhy, Effort, EffortInUse, EffortSource, ErrorClass, ModelChanged,
    TransientBody,
};
use miyu_kernel::session::Command;
use miyu_session::{ConfigSource, Handle, Models, Pushed};
use miyu_tool::Catalog;
use support::routing::{called, configs, hellos, routes};
use support::{Home, Lines, Opening, ask, say, stop, until_turn_ends, watch};

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

fn source(text: &str) -> Arc<dyn ConfigSource> {
    Arc::clone(&*configs(text, &[]).borrow())
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

/// 换会话给 `model` 记的那一档（`None` 清掉）。
async fn effort(handle: &Handle, command: &str, model: &str, level: Option<&str>) {
    let configure = Command::Configure {
        model: None,
        effort: Some(Effort {
            model: model.to_string(),
            level: level.map(str::to_string),
        }),
    };
    ask(handle, command, configure).await.expect("会话在跑");
}

/// 换模型。
async fn switch(handle: &Handle, command: &str, model: &str) {
    let configure = Command::Configure {
        model: Some(model.to_string()),
        effort: None,
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
async fn a_request_takes_the_session_cell_then_the_config_then_nothing() {
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
        used("low", EffortSource::Config),
        "造好就看得到"
    );
    turn(&handle, "cmd-1").await;
    assert_eq!(
        sent(&first).first().map(String::as_str),
        Some("low"),
        "配置的默认"
    );
    effort(&handle, "cmd-2", "a/m", Some("high")).await;
    assert_eq!(
        handle.next().effort,
        used("low", EffortSource::Config),
        "下一个回合开始才换"
    );
    let changed = turn(&handle, "cmd-3").await;
    assert_eq!(changed.len(), 1, "强度变了推一条");
    assert_eq!(changed[0].why, ChangeWhy::Turn);
    assert_eq!(changed[0].effort, used("high", EffortSource::Session));
    assert_eq!(handle.next().effort, used("high", EffortSource::Session));
    assert_eq!(
        sent(&first).last().map(String::as_str),
        Some("high"),
        "会话的那一格"
    );
    // 清掉：回到配置的默认。
    effort(&handle, "cmd-4", "a/m", None).await;
    let changed = turn(&handle, "cmd-5").await;
    assert_eq!(changed[0].effort, used("low", EffortSource::Config));
    assert_eq!(sent(&first).last().map(String::as_str), Some("low"));
    // 一样的强度、一样的模型：不推。
    assert!(turn(&handle, "cmd-6").await.is_empty());
    // 换模型以后用新模型自己的：`b/n` 没有默认、会话也没记，什么都不带。
    switch(&handle, "cmd-7", "b/n").await;
    let changed = turn(&handle, "cmd-8").await;
    assert_eq!(changed[0].effort, None);
    assert_eq!(
        sent(&second).last().map(String::as_str),
        Some("-"),
        "什么都不带"
    );
    effort(&handle, "cmd-9", "b/n", Some("max")).await;
    turn(&handle, "cmd-10").await;
    assert_eq!(sent(&second).last().map(String::as_str), Some("max"));
    assert!(
        sent(&first).iter().all(|level| level != "max"),
        "a 那一家没收到过 b 的那一档"
    );
}

#[tokio::test]
async fn each_member_of_a_rotating_pool_uses_its_own_cell() {
    let (first, second) = (
        Server::start(hellos(8)).await,
        Server::start(hellos(8)).await,
    );
    let mut home = Home::new();
    home.configs = configs(&config(&first, &second, A_LEVELS), &[]);
    let routes = routes(serde_json::json!({}), Duration::from_secs(5));
    let handle = create(&home, &routes, "@p").await;
    effort(&handle, "cmd-1", "a/m", Some("off")).await;
    effort(&handle, "cmd-2", "b/n", Some("max")).await;
    let changed = turn(&handle, "cmd-3").await;
    assert!(
        changed.iter().all(|changed| changed.effort.is_none()),
        "轮换的池没有单一的模型，不带"
    );
    turn(&handle, "cmd-4").await;
    assert_eq!(handle.next().effort, None);
    let (a, b) = (sent(&first), sent(&second));
    assert!(
        !a.is_empty() && !b.is_empty(),
        "两个成员都发过：{a:?} {b:?}"
    );
    assert!(
        a.iter().all(|level| level == "none"),
        "a/m 记的是 off，没有开关的发 none：{a:?}"
    );
    assert!(b.iter().all(|level| level == "max"), "{b:?}");
}

#[tokio::test]
async fn a_cell_the_model_no_longer_offers_falls_back_to_the_config() {
    let (first, second) = (
        Server::start(hellos(8)).await,
        Server::start(hellos(8)).await,
    );
    let (switching, receiving) = channel::channel(source(&config(&first, &second, A_LEVELS)));
    let mut home = Home::new();
    home.configs = receiving;
    let routes = routes(serde_json::json!({}), Duration::from_secs(5));
    let handle = create(&home, &routes, "a/m").await;
    effort(&handle, "cmd-1", "a/m", Some("high")).await;
    turn(&handle, "cmd-2").await;
    assert_eq!(sent(&first).first().map(String::as_str), Some("high"));
    // 目录变了：`high` 不在了。
    switching.send_replace(source(&config(&first, &second, "\"off\", \"low\"")));
    let changed = turn(&handle, "cmd-3").await;
    assert_eq!(
        sent(&first).last().map(String::as_str),
        Some("low"),
        "照配置的默认"
    );
    assert_eq!(changed[0].effort, used("low", EffortSource::Config));
    assert!(
        home.log(handle.id()).iter().all(|event| !matches!(
            &event.body,
            miyu_kernel::event::Body::PolicyChanged(changed)
                if changed.effort.as_ref().is_some_and(|effort| effort.level.is_none())
        )),
        "日志里那一格不改"
    );
}

/// 载入的会话照日志拼的那一格造路由：第一轮开始以前给头看的就是它。
#[tokio::test]
async fn a_loaded_session_uses_the_cells_from_its_log() {
    let (first, second) = (
        Server::start(Vec::new()).await,
        Server::start(Vec::new()).await,
    );
    let mut home = Home::new();
    home.configs = configs(&config(&first, &second, A_LEVELS), &[]);
    let routes = routes(serde_json::json!({}), Duration::from_secs(5));
    let handle = create(&home, &routes, "a/m").await;
    effort(&handle, "cmd-1", "a/m", Some("off")).await;
    stop(&handle).await;
    let loaded = home.load(handle.id(), &routes).await;
    assert_eq!(loaded.next().effort, used("off", EffortSource::Session));
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
