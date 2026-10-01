//! 回合开始时重新解析（`docs/blueprint/models.md`「怎么走」第六条，`session/actor.md` 第 8 条，施工 8-10）：换了模型的下一轮
//! 发给新的、推 `model.changed`（`why` 是 `turn`）、限额和给头看的跟着换；钉着的没了退回这一轮的 `models.chat`，内核记一条、
//! 以后钉在它上面；`models.chat` 也没有的不记、当场 `no_model`；只改了窗口的下一轮用上；换成轮换的池推的没有端点；载入
//! 以后照换过的说。

mod support;

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::watch as channel;

use miyu_http::testkit::Server;
use miyu_kernel::event::{Body, ChangeWhy, ErrorClass, ModelChanged, PolicyChanged, TransientBody};
use miyu_kernel::session::Command;
use miyu_session::{ConfigSource, Handle, Models, Pushed};
use miyu_tool::Catalog;
use support::routing::{called, configs, hellos, routes};
use support::{Home, Lines, Opening, ask, say, stop, until_turn_ends, watch};

/// 两家 `a`、`b`，都不带 key；`b` 的 `n` 窗口 32000，`a` 的 `m` 窗口 `window`。`models.chat` 是 `chat`；池 `p` 是 `a/m`、
/// `b/n`，轮换。`only_b` 的，配置里没有 `a`、池。
fn config(first: &Server, second: &Server, window: u64, chat: &str, only_b: bool) -> String {
    let a = format!(
        "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[providers.a.models.m]\nwindow = {window}\n\n",
        first.base_url
    );
    let pool = "[pools.p]\nmodels = [\"a/m\", \"b/n\"]\nstrategy = \"rotate\"\n";
    let b = format!(
        "[providers.b]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[providers.b.models.n]\nwindow = 32000\n\n[models]\nchat = \"{chat}\"\n\n",
        second.base_url
    );
    match only_b {
        true => b,
        false => format!("{a}{b}{pool}"),
    }
}

/// 照字造一份配置的来源：换配置时整份换上。
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

/// 说一句、等这一轮说完，交回这一轮推过来的每一份。
async fn turn(handle: &Handle, command: &str) -> Vec<Arc<Pushed>> {
    let mut pushes = watch(handle).await;
    ask(handle, command, say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await
}

/// 推过来的 `model.changed`。
fn changes(pushed: &[Arc<Pushed>]) -> Vec<ModelChanged> {
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

/// 主请求发给了哪一家，照先后。
fn endpoints(home: &Home, handle: &Handle) -> Vec<String> {
    called(home, handle)
        .iter()
        .filter(|call| call.purpose.is_none())
        .map(|call| {
            call.endpoint
                .as_ref()
                .map_or_else(|| "-".to_string(), |e| e.as_str().to_string())
        })
        .collect()
}

/// 日志里换模型的几条：换成的、原来的、是不是内核写的。
fn model_changes(home: &Home, handle: &Handle) -> Vec<(String, Option<String>, bool)> {
    home.log(handle.id())
        .into_iter()
        .filter_map(|event| match event.body {
            Body::PolicyChanged(PolicyChanged {
                model: Some(model),
                replaced,
                ..
            }) => Some((model, replaced, event.turn.is_some())),
            _ => None,
        })
        .collect()
}

/// 换成 `model`。
async fn configure(handle: &Handle, command: &str, model: &str) {
    let configure = Command::Configure {
        model: Some(model.to_string()),
        effort: None,
    };
    ask(handle, command, configure).await.expect("会话在跑");
}

#[tokio::test]
async fn a_new_model_is_used_from_the_next_turn_and_announced_once() {
    let (first, second) = (
        Server::start(hellos(8)).await,
        Server::start(hellos(8)).await,
    );
    let mut home = Home::new();
    home.configs = configs(&config(&first, &second, 64_000, "a/m", false), &[]);
    let routes = routes(serde_json::json!({}), Duration::from_secs(5));
    let handle = create(&home, &routes, "a/m").await;
    turn(&handle, "cmd-1").await;
    configure(&handle, "cmd-2", "b/n").await;
    assert_eq!(
        handle.next().reference.as_deref(),
        Some("a/m"),
        "下一个回合开始才换"
    );
    let pushed = turn(&handle, "cmd-3").await;
    let changed = changes(&pushed);
    assert_eq!(changed.len(), 1, "回合开始推一条");
    assert_eq!(changed[0].why, ChangeWhy::Turn);
    assert_eq!(changed[0].reference.as_deref(), Some("b/n"));
    assert_eq!(
        (
            changed[0].endpoint.as_ref().map(|e| e.as_str()),
            changed[0].model.as_ref().map(|m| m.as_str())
        ),
        (Some("b"), Some("n"))
    );
    assert_eq!(changed[0].limits.window, Some(32_000));
    assert_eq!(handle.limits().window, Some(32_000), "订阅时交的限额跟着换");
    let next = handle.next();
    assert_eq!(
        (
            next.reference.as_deref(),
            next.model.map(|m| format!("{}/{}", m.endpoint, m.model))
        ),
        (Some("b/n"), Some("b/n".to_string()))
    );
    let pushed = turn(&handle, "cmd-4").await;
    assert!(changes(&pushed).is_empty(), "没再变的不推");
    assert_eq!(endpoints(&home, &handle), ["a", "b", "b"]);
    assert_eq!(
        model_changes(&home, &handle),
        [("b/n".to_string(), None, false)]
    );
}

#[tokio::test]
async fn a_gone_model_falls_back_to_models_chat_and_stays_there() {
    let (first, second) = (
        Server::start(hellos(8)).await,
        Server::start(hellos(8)).await,
    );
    let (switch, receiving) =
        channel::channel(source(&config(&first, &second, 64_000, "b/n", false)));
    let mut home = Home::new();
    home.configs = receiving;
    let routes = routes(serde_json::json!({}), Duration::from_secs(5));
    let handle = create(&home, &routes, "a/m").await;
    turn(&handle, "cmd-1").await;
    switch.send_replace(source(&config(&first, &second, 64_000, "b/n", true)));
    let pushed = turn(&handle, "cmd-2").await;
    assert_eq!(
        model_changes(&home, &handle),
        [("b/n".to_string(), Some("a/m".to_string()), true)],
        "内核在那一轮里记下退回的"
    );
    let changed = changes(&pushed);
    assert_eq!(changed.len(), 1);
    assert_eq!(
        (changed[0].reference.as_deref(), changed[0].why.clone()),
        (Some("b/n"), ChangeWhy::Turn)
    );
    // `a` 回来了：照样钉在退回的上面。
    switch.send_replace(source(&config(&first, &second, 64_000, "b/n", false)));
    turn(&handle, "cmd-3").await;
    assert_eq!(endpoints(&home, &handle), ["a", "b", "b"]);
}

#[tokio::test]
async fn nothing_is_recorded_when_models_chat_is_gone_too() {
    let (first, second) = (
        Server::start(hellos(4)).await,
        Server::start(hellos(4)).await,
    );
    let (switch, receiving) =
        channel::channel(source(&config(&first, &second, 64_000, "a/m", false)));
    let mut home = Home::new();
    home.configs = receiving;
    let routes = routes(serde_json::json!({}), Duration::from_secs(5));
    let handle = create(&home, &routes, "a/m").await;
    turn(&handle, "cmd-1").await;
    // 只剩 `b`，`models.chat` 还指着没了的 `a`。
    switch.send_replace(source(&config(&first, &second, 64_000, "a/m", true)));
    turn(&handle, "cmd-2").await;
    assert!(model_changes(&home, &handle).is_empty(), "不记");
    let calls = called(&home, &handle);
    let last = calls.last().expect("有一条");
    assert_eq!(
        last.error.as_ref().map(|error| error.class.clone()),
        Some(ErrorClass::NoModel),
        "这一轮的请求当场 no_model"
    );
    assert_eq!(handle.next().reference.as_deref(), Some("a/m"));
    assert_eq!(handle.next().model, None, "解析不出的没有模型");
}

#[tokio::test]
async fn a_new_window_reaches_an_open_session_at_the_next_turn() {
    let (first, second) = (
        Server::start(hellos(4)).await,
        Server::start(Vec::new()).await,
    );
    let (switch, receiving) =
        channel::channel(source(&config(&first, &second, 64_000, "a/m", false)));
    let mut home = Home::new();
    home.configs = receiving;
    let routes = routes(serde_json::json!({}), Duration::from_secs(5));
    let handle = create(&home, &routes, "a/m").await;
    turn(&handle, "cmd-1").await;
    assert_eq!(handle.limits().window, Some(64_000));
    switch.send_replace(source(&config(&first, &second, 48_000, "a/m", false)));
    assert_eq!(handle.limits().window, Some(64_000), "开着的会话这一刻不变");
    let pushed = turn(&handle, "cmd-2").await;
    assert_eq!(handle.limits().window, Some(48_000), "下一个回合开始用上");
    let changed = changes(&pushed);
    assert_eq!(changed.len(), 1, "只改了窗口也推");
    assert_eq!(changed[0].limits.window, Some(48_000));
    assert_eq!(changed[0].reference.as_deref(), Some("a/m"));
}

#[tokio::test]
async fn a_rotating_pool_is_announced_without_an_endpoint() {
    let (first, second) = (
        Server::start(hellos(4)).await,
        Server::start(hellos(4)).await,
    );
    let mut home = Home::new();
    home.configs = configs(&config(&first, &second, 64_000, "a/m", false), &[]);
    let routes = routes(serde_json::json!({}), Duration::from_secs(5));
    let handle = create(&home, &routes, "a/m").await;
    configure(&handle, "cmd-1", "@p").await;
    let pushed = turn(&handle, "cmd-2").await;
    let changed = changes(&pushed);
    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0].reference.as_deref(), Some("@p"));
    assert_eq!(
        (&changed[0].endpoint, &changed[0].model),
        (&None, &None),
        "轮换的池每次都换"
    );
    assert_eq!(changed[0].limits.window, Some(32_000), "取成员里小的");
    assert_eq!(handle.next().model, None);
}

#[tokio::test]
async fn a_loaded_session_speaks_of_the_model_it_was_switched_to() {
    let (first, second) = (
        Server::start(Vec::new()).await,
        Server::start(Vec::new()).await,
    );
    let mut home = Home::new();
    home.configs = configs(&config(&first, &second, 64_000, "a/m", false), &[]);
    let routes = routes(serde_json::json!({}), Duration::from_secs(5));
    let handle = create(&home, &routes, "a/m").await;
    configure(&handle, "cmd-1", "b/n").await;
    stop(&handle).await;
    let loaded = home.load(handle.id(), &routes).await;
    assert_eq!(
        loaded.next().reference.as_deref(),
        Some("b/n"),
        "载入照日志算的引用造路由"
    );
    assert_eq!(loaded.limits().window, Some(32_000));
}
