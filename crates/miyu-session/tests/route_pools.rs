//! 会话的路由里的池（`docs/blueprint/models.md`「守着它的」`route.rs` 那一行 8-8 的一半，施工 8-8）：两台假服务器各是一家。
//! 钉住的池一个会话一直发给一个成员，新会话照指针分开；载入时照最近一条发出去了的 `model.called` 认回钉着的；指针写进
//! `state/models/pools.json`；轮换的每次请求换下一个；这时用不了的成员跳过、钉到下一个；池没了退回 `models.chat`；限额照
//! 钉着的成员，轮换的取小的；`session.created` 记下会话的引用。

mod support;

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::watch;

use miyu_config::secret::Reference;
use miyu_http::testkit::Server;
use miyu_kernel::event::Body;
use miyu_models::matching::Vendors;
use miyu_models::profile::Profiles;
use miyu_session::{ConfigSource, Handle, ModelData, Models, read_observed};
use miyu_tool::Catalog;
use support::routing::{called, configs, hellos, routes, routes_with, turn};
use support::{Home, Lines, Opening, Scratch, stop, within};

/// 两家：`a` 在 `first`、`b` 在 `second`，都不带 key。池 `p` 的成员照 `members` 写，分法照 `strategy`（空的不写）；
/// `models.chat` 是 `a/m`。`a` 的 `x` 窗口 64000、最大输出 8000，`b` 的 `y` 窗口 32000、最大输出 16000。
fn two(first: &Server, second: &Server, members: &str, strategy: &str) -> String {
    let strategy = match strategy {
        "" => String::new(),
        written => format!("strategy = \"{written}\"\n"),
    };
    format!(
        "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[providers.a.models.x]\nwindow = 64000\nmax_output = 8000\n\n[providers.b]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[providers.b.models.y]\nwindow = 32000\nmax_output = 16000\n\n[models]\nchat = \"a/m\"\n\n[pools.p]\nmodels = [{members}]\n{strategy}",
        first.base_url, second.base_url
    )
}

/// 造一个会话，记着的模型是 `model`（没有的照 `models.chat`）。
async fn create(home: &Home, models: &dyn Models, model: Option<&str>) -> Handle {
    let lines = Lines {
        model: model.map(str::to_string),
        ..Lines::default()
    };
    home.create_full(models, &Catalog::default(), Opening::default(), lines)
        .await
}

/// 主请求（不算起标题这类辅助请求）发给了哪一家的哪个模型（`<供应商>/<模型>`），照先后。
fn sent(home: &Home, handle: &Handle) -> Vec<String> {
    called(home, handle)
        .iter()
        .filter(|call| call.purpose.is_none())
        .filter_map(|call| {
            Some(format!(
                "{}/{}",
                call.endpoint.as_ref()?,
                call.model.as_ref()?
            ))
        })
        .collect()
}

/// 会话的 `session.created` 记着的模型。
fn recorded(home: &Home, handle: &Handle) -> Option<String> {
    match &home.log(handle.id())[0].body {
        Body::SessionCreated(created) => created.model.clone(),
        other => panic!("第 1 条是造会话：{other:?}"),
    }
}

/// 一份能写 `state/models/` 的模型资料，目录是 `dir`；起来时照它读回来的。
fn kept(dir: &std::path::Path) -> Arc<ModelData> {
    let data = ModelData::new(
        Profiles::parse(&serde_json::json!({})).expect("档案写法对"),
        Vendors::default(),
        Some(dir.to_path_buf()),
    );
    data.loaded(None, read_observed(dir));
    Arc::new(data)
}

#[tokio::test]
async fn a_pinned_pool_keeps_a_session_on_one_member_and_spreads_new_sessions() {
    let (first, second) = (
        Server::start(hellos(8)).await,
        Server::start(hellos(8)).await,
    );
    let mut home = Home::new();
    home.configs = configs(&two(&first, &second, r#""gone/z", "a/x", "b/y""#, ""), &[]);
    let routes = routes(serde_json::json!({}), Duration::from_secs(60));
    let one = create(&home, &routes, Some("@p")).await;
    let other = create(&home, &routes, Some("@p")).await;
    for n in 1..=2 {
        turn(&one, &format!("cmd-{n}")).await;
        turn(&other, &format!("cmd-{n}")).await;
    }
    assert_eq!(
        sent(&home, &one),
        ["a/x", "a/x"],
        "认不出的成员跳过，第一个会话钉在第一个"
    );
    assert_eq!(
        sent(&home, &other),
        ["b/y", "b/y"],
        "指针往前走，第二个会话钉在下一个"
    );
    assert_eq!(recorded(&home, &one).as_deref(), Some("@p"));
    assert_eq!(one.limits().window, Some(64_000), "限额照钉着的成员");
    assert_eq!(other.limits().window, Some(32_000));
}

#[tokio::test]
async fn the_pinned_member_comes_back_from_the_log_and_the_pointer_from_disk() {
    let (first, second) = (
        Server::start(hellos(8)).await,
        Server::start(hellos(8)).await,
    );
    let state = Scratch::new();
    std::fs::create_dir_all(state.0.as_path()).expect("建得了");
    let mut home = Home::new();
    home.configs = configs(&two(&first, &second, r#""a/x", "b/y""#, "pin"), &[]);
    let routes = routes_with(kept(state.0.as_path()), Duration::from_secs(60));
    let handle = create(&home, &routes, Some("@p")).await;
    turn(&handle, "cmd-1").await;
    // 指针写进了 `state/models/pools.json`：走过一个，下一个是第 1 个。
    let file = state.0.as_path().join("pools.json");
    within("写指针", async {
        while std::fs::read_to_string(&file).ok().as_deref() != Some(r#"{"p":1}"#) {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
    // 重启：新的路由照磁盘读回指针（下一个是 b），载入的会话照日志认回钉着的 a。
    let session = handle.id().clone();
    stop(&handle).await;
    let again = routes_with(kept(state.0.as_path()), Duration::from_secs(60));
    let loaded = home.load(&session, &again).await;
    turn(&loaded, "cmd-2").await;
    assert_eq!(sent(&home, &loaded), ["a/x", "a/x"], "载入以后还是它");
    // 新会话照读回来的指针：b。
    let fresh = create(&home, &again, Some("@p")).await;
    turn(&fresh, "cmd-1").await;
    assert_eq!(sent(&home, &fresh), ["b/y"]);
}

#[tokio::test]
async fn a_rotating_pool_takes_the_next_member_for_every_request() {
    let (first, second) = (
        Server::start(hellos(8)).await,
        Server::start(hellos(8)).await,
    );
    let mut home = Home::new();
    home.configs = configs(&two(&first, &second, r#""a/x", "b/y""#, "rotate"), &[]);
    let routes = routes(serde_json::json!({}), Duration::from_secs(60));
    let handle = create(&home, &routes, Some("@p")).await;
    for n in 1..=3 {
        turn(&handle, &format!("cmd-{n}")).await;
    }
    let (a, b) = (first.received().len(), second.received().len());
    assert!(
        b >= 1 && a >= b && a - b <= 1,
        "一次一个，轮着来：a {a}，b {b}"
    );
    assert_eq!(sent(&home, &handle)[0], "a/x", "从指针指的第一个起");
    assert_eq!(handle.limits().window, Some(32_000), "轮换的取窗口小的");
}

#[tokio::test]
async fn a_member_that_cannot_be_used_now_is_passed_over_and_the_pin_moves() {
    let (first, second) = (
        Server::start(hellos(8)).await,
        Server::start(hellos(8)).await,
    );
    let text = two(&first, &second, r#""a/x", "b/y""#, "pin").replace(
        "[providers.a.models.x]",
        "keys = [{ env = \"KA\" }]\n\n[providers.a.models.x]",
    );
    let source = |secrets: &[(Reference, &str)]| -> Arc<dyn ConfigSource> {
        Arc::clone(&*configs(&text, secrets).borrow())
    };
    let (switch, receiving) = watch::channel(source(&[]));
    let mut home = Home::new();
    home.configs = receiving;
    let routes = routes(serde_json::json!({}), Duration::from_secs(60));
    let handle = create(&home, &routes, Some("@p")).await;
    turn(&handle, "cmd-1").await;
    // `a` 的 key 有了：钉着的已经换成 b，不回去。
    switch.send_replace(source(&[(Reference::Env("KA".to_string()), "sk-a")]));
    turn(&handle, "cmd-2").await;
    assert_eq!(sent(&home, &handle), ["b/y", "b/y"]);
    assert!(first.received().is_empty());
}

#[tokio::test]
async fn a_pool_that_is_gone_falls_back_to_the_chat_model() {
    let (first, second) = (
        Server::start(hellos(8)).await,
        Server::start(hellos(8)).await,
    );
    let with = two(&first, &second, r#""b/y""#, "");
    let without = with[..with.find("[pools.p]").expect("有池")].to_string();
    let source =
        |text: &str| -> Arc<dyn ConfigSource> { Arc::clone(&*configs(text, &[]).borrow()) };
    let (switch, receiving) = watch::channel(source(&with));
    let mut home = Home::new();
    home.configs = receiving;
    let routes = routes(serde_json::json!({}), Duration::from_secs(60));
    let handle = create(&home, &routes, Some("@p")).await;
    turn(&handle, "cmd-1").await;
    switch.send_replace(source(&without));
    turn(&handle, "cmd-2").await;
    switch.send_replace(source(&with));
    turn(&handle, "cmd-3").await;
    assert_eq!(
        sent(&home, &handle),
        ["b/y", "a/m", "a/m"],
        "退回 chat 以后钉在它上面"
    );
}

#[tokio::test]
async fn session_created_records_the_reference_it_was_made_with() {
    let (first, second) = (
        Server::start(hellos(1)).await,
        Server::start(hellos(1)).await,
    );
    let mut home = Home::new();
    home.configs = configs(&two(&first, &second, r#""b/y""#, ""), &[]);
    let routes = routes(serde_json::json!({}), Duration::from_secs(60));
    let given = create(&home, &routes, Some("b/y")).await;
    assert_eq!(recorded(&home, &given).as_deref(), Some("b/y"));
    let default = create(&home, &routes, None).await;
    assert_eq!(
        recorded(&home, &default).as_deref(),
        Some("a/m"),
        "没指定的照这时的 chat"
    );
    let bare = Home::new();
    let nothing = create(&bare, &routes, None).await;
    assert_eq!(recorded(&bare, &nothing), None, "chat 也没配的不写");
}
