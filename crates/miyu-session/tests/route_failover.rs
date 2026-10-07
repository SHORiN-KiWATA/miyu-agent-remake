//! 会话的路由出错换端点（`docs/blueprint/models.md`「守着它的」`route.rs`、`route_pools.rs` 那一行 8-9 的一半，「怎么走」
//! 第四条、第五条，施工 8-9）：假服务器回 429，换到别的 key、池里的下一个当场再来，成了以后会话钉在它上面；说到一半断了的
//! 还发给原来那一个；只有一个候选的照旧在它上面再来；全在冷却的不发、交 `cooling`、原话列出每个候选；换端点数进 5 次；
//! 不换的几类不记冷却；钉着的成员换了推 `model.changed`、限额跟着换，换 key 不推。

mod support;

use std::sync::Arc;
use std::time::Duration;

use miyu_config::secret::Reference;
use miyu_http::testkit::{Reply, Server};
use miyu_kernel::event::{Body, ChangeWhy, ErrorClass, ModelChanged, Status, TransientBody};
use miyu_models::cooldown::{Rule, Rules};
use miyu_models::keys;
use miyu_session::{Handle, Models, Pushed};
use miyu_tool::Catalog;
use support::routing::{called, configs, cut_after, hellos, routes};
use support::{Home, Lines, Opening, ask, say, until_turn_ends, watch};

/// 一家 `a` 在 `base_url`，`count` 个 key 照 `{ env = "K<n>" }` 写，值是 `sk-<n>`，都取得到；`models.chat` 是 `a/m`。
fn keyed(base_url: &str, count: usize) -> (String, Vec<(Reference, String)>) {
    let refs: Vec<String> = (1..=count)
        .map(|n| format!("{{ env = \"K{n}\" }}"))
        .collect();
    let text = format!(
        "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"{base_url}\"\nkeys = [{}]\n\n[models]\nchat = \"a/m\"\n",
        refs.join(", ")
    );
    let secrets = (1..=count)
        .map(|n| (Reference::Env(format!("K{n}")), format!("sk-{n}")))
        .collect();
    (text, secrets)
}

/// 照 [`keyed`] 的配置。
fn keyed_configs(base_url: &str, count: usize) -> miyu_session::Configs {
    let (text, secrets) = keyed(base_url, count);
    let secrets: Vec<(Reference, &str)> = secrets
        .iter()
        .map(|(reference, value)| (reference.clone(), value.as_str()))
        .collect();
    configs(&text, &secrets)
}

/// 两家：`a` 在 `first`、`b` 在 `second`，都不带 key。池 `p` 是 `a/x`、`b/y`，分法 `strategy`。`a` 的 `x` 窗口 64000，
/// `b` 的 `y` 窗口 32000。
fn pooled(first: &Server, second: &Server, strategy: &str) -> String {
    format!(
        "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[providers.a.models.x]\nwindow = 64000\n\n[providers.b]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[providers.b.models.y]\nwindow = 32000\n\n[models]\nchat = \"a/m\"\n\n[pools.p]\nmodels = [\"a/x\", \"b/y\"]\nstrategy = \"{strategy}\"\n",
        first.base_url, second.base_url
    )
}

/// 一次限速：429，没说等多久。
fn limited() -> Reply {
    Reply::error(429, &[], r#"{"error":{"message":"Rate limit reached"}}"#)
}

/// 冷却的规矩：三类都照 `rule`。
fn rules(rule: Rule) -> Rules {
    Rules {
        rate_limited: rule,
        retryable: rule,
        auth: rule,
    }
}

/// 冷却 `base` 秒起、最多 `max` 秒。
fn rule(base: u64, max: u64) -> Rule {
    Rule {
        base: Duration::from_secs(base),
        max: Duration::from_secs(max),
    }
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

/// 说一句、等这一轮说完，交回这一轮推过来的每一份。
async fn turn(handle: &Handle, command: &str) -> Vec<Arc<Pushed>> {
    let mut pushes = watch(handle).await;
    ask(handle, command, say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await
}

/// 推过来的重试状态。
fn statuses(pushed: &[Arc<Pushed>]) -> Vec<Status> {
    pushed
        .iter()
        .filter_map(|pushed| match &**pushed {
            Pushed::Transient(transient) => match &transient.body {
                TransientBody::Status(status) => Some(status.clone()),
                _ => None,
            },
            Pushed::Events(_) => None,
        })
        .collect()
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

/// 服务器收到的每个请求的认证头，照先后。
fn bearers(server: &Server) -> Vec<String> {
    server
        .received()
        .iter()
        .map(|received| received.header("authorization").unwrap_or("").to_string())
        .collect()
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

/// 主请求出错的分类，照先后。
fn failures(home: &Home, handle: &Handle) -> Vec<String> {
    called(home, handle)
        .iter()
        .filter(|call| call.purpose.is_none())
        .filter_map(|call| {
            call.error
                .as_ref()
                .map(|error| error.class.as_str().to_string())
        })
        .collect()
}

#[tokio::test]
async fn a_429_moves_to_the_next_key_at_once_and_the_session_stays_on_it() {
    let mut replies = vec![limited()];
    replies.extend(hellos(6));
    let server = Server::start(replies).await;
    let mut home = Home::new();
    home.configs = keyed_configs(&server.base_url, 2);
    let routes = routes(serde_json::json!({}), Duration::from_secs(60));
    routes.data.set_cooldown_rules(rules(rule(1, 1)));
    let handle = home.create(&routes).await;
    let pinned = keys::pinned(handle.id().as_str(), 2).expect("有 key");
    let (own, other) = (
        format!("Bearer sk-{}", pinned + 1),
        format!("Bearer sk-{}", 2 - pinned),
    );
    let pushed = turn(&handle, "cmd-1").await;
    let status = statuses(&pushed);
    assert_eq!(status.len(), 1);
    assert!(status[0].retry.failover, "换了端点的状态带 failover");
    assert_eq!(status[0].retry.wait_ms, 0, "当场再来");
    assert_eq!(status[0].retry.class, ErrorClass::RateLimited);
    assert!(
        changes(&pushed).is_empty(),
        "换 key、模型没变的不推 model.changed"
    );
    assert_eq!(failures(&home, &handle), ["rate_limited"]);
    // 钉着的那个冷却 1 秒；过了冷却，这个会话还用换过去的那一个（成了才换，换了就一直用它）。
    tokio::time::sleep(Duration::from_millis(1100)).await;
    turn(&handle, "cmd-2").await;
    let bearers = bearers(&server);
    assert_eq!(bearers[0], own, "先发给照会话编号钉着的");
    assert!(bearers.len() >= 3);
    assert!(
        bearers[1..].iter().all(|bearer| *bearer == other),
        "换过去以后一直是它：{bearers:?}"
    );
}

#[tokio::test]
async fn a_429_in_a_pinned_pool_moves_on_and_pins_the_member_that_worked() {
    let (first, second) = (
        Server::start(vec![limited()]).await,
        Server::start(hellos(8)).await,
    );
    let mut home = Home::new();
    home.configs = configs(&pooled(&first, &second, "pin"), &[]);
    let routes = routes(serde_json::json!({}), Duration::from_secs(60));
    let handle = create(&home, &routes, Some("@p")).await;
    assert_eq!(handle.limits().window, Some(64_000), "先钉着 a 的");
    let pushed = turn(&handle, "cmd-1").await;
    assert!(statuses(&pushed)[0].retry.failover);
    let changed = changes(&pushed);
    assert_eq!(changed.len(), 1, "成了以后推一条");
    assert_eq!(changed[0].reference.as_deref(), Some("@p"));
    assert_eq!(
        (
            changed[0].endpoint.as_ref().map(|e| e.as_str()),
            changed[0].model.as_ref().map(|m| m.as_str())
        ),
        (Some("b"), Some("y"))
    );
    assert_eq!(changed[0].limits.window, Some(32_000));
    assert_eq!(changed[0].why, ChangeWhy::Failover);
    assert_eq!(handle.limits().window, Some(32_000), "订阅时交的限额跟着换");
    let pushed = turn(&handle, "cmd-2").await;
    assert!(changes(&pushed).is_empty(), "没再换的不推");
    assert_eq!(
        sent(&home, &handle),
        ["a/x", "b/y", "b/y"],
        "成了以后钉在 b 上"
    );
    assert_eq!(first.received().len(), 1);
}

#[tokio::test]
async fn a_rotating_pool_passes_over_a_cooling_member() {
    let (first, second) = (
        Server::start(vec![limited()]).await,
        Server::start(hellos(12)).await,
    );
    let mut home = Home::new();
    home.configs = configs(&pooled(&first, &second, "rotate"), &[]);
    let routes = routes(serde_json::json!({}), Duration::from_secs(60));
    let handle = create(&home, &routes, Some("@p")).await;
    let mut pushed = Vec::new();
    for n in 1..=3 {
        pushed.extend(turn(&handle, &format!("cmd-{n}")).await);
    }
    assert_eq!(first.received().len(), 1, "a 在冷却：轮到它也跳过");
    assert_eq!(sent(&home, &handle)[..2], ["a/x", "b/y"]);
    assert!(
        sent(&home, &handle)[1..].iter().all(|sent| sent == "b/y"),
        "{:?}",
        sent(&home, &handle)
    );
    assert!(changes(&pushed).is_empty(), "轮换的池不推 model.changed");
}

#[tokio::test]
async fn a_reply_cut_midway_goes_back_to_the_same_endpoint() {
    let mut replies = vec![cut_after(2)];
    replies.extend(hellos(6));
    let (first, second) = (Server::start(replies).await, Server::start(hellos(6)).await);
    let mut home = Home::new();
    home.configs = configs(&pooled(&first, &second, "pin"), &[]);
    let routes = routes(serde_json::json!({}), Duration::from_secs(60));
    let handle = create(&home, &routes, Some("@p")).await;
    let pushed = turn(&handle, "cmd-1").await;
    assert_eq!(failures(&home, &handle), ["retryable"]);
    assert_eq!(
        sent(&home, &handle),
        ["a/x", "a/x"],
        "说到一半断了：不管它冷不冷，还发给它，接着说"
    );
    assert!(!statuses(&pushed)[0].retry.failover, "没换端点");
    assert!(second.received().is_empty());
}

#[tokio::test]
async fn a_single_candidate_is_asked_again_on_itself() {
    let mut replies = vec![Reply::error(429, &[("retry-after", "1")], "{}")];
    replies.extend(hellos(4));
    let server = Server::start(replies).await;
    let mut home = Home::new();
    home.configs = keyed_configs(&server.base_url, 1);
    let routes = routes(serde_json::json!({}), Duration::from_secs(60));
    let handle = home.create(&routes).await;
    let pushed = turn(&handle, "cmd-1").await;
    let status = statuses(&pushed);
    assert!(!status[0].retry.failover, "只有一个候选：不算换");
    assert_eq!(status[0].retry.wait_ms, 1000, "照供应商说的等");
    assert_eq!(sent(&home, &handle), ["a/m", "a/m"], "冷却着也照发它");
}

#[tokio::test]
async fn all_candidates_cooling_says_cooling_and_names_each_one() {
    let server = Server::start(vec![limited(), limited()]).await;
    let mut home = Home::new();
    home.configs = keyed_configs(&server.base_url, 2);
    let routes = routes(serde_json::json!({}), Duration::from_secs(60));
    // 冷却 10 分钟起：两个 key 都限速以后，要等的超过 2 分钟，内核不等。
    routes.data.set_cooldown_rules(rules(rule(600, 3600)));
    let handle = home.create(&routes).await;
    let pushed = turn(&handle, "cmd-1").await;
    assert!(statuses(&pushed)[0].retry.failover, "先换到另一个 key");
    assert_eq!(failures(&home, &handle), ["rate_limited", "rate_limited"]);
    turn(&handle, "cmd-2").await;
    assert_eq!(server.received().len(), 2, "全在冷却：不发");
    let calls = called(&home, &handle);
    let last = calls.last().expect("有一条");
    let error = last.error.as_ref().expect("出错");
    assert_eq!(error.class, ErrorClass::Cooling);
    assert_eq!(last.endpoint, None, "没发出去，没有端点");
    assert!(
        error
            .message
            .starts_with("all candidates cooling: a/m key "),
        "{}",
        error.message
    );
    for key in [
        "a/m key 1 rate_limited until 20",
        "a/m key 2 rate_limited until 20",
    ] {
        assert!(error.message.contains(key), "{}", error.message);
    }
}

#[tokio::test]
async fn failovers_count_toward_the_five_retries() {
    let server = Server::start(vec![limited(); 7]).await;
    let mut home = Home::new();
    home.configs = keyed_configs(&server.base_url, 7);
    let routes = routes(serde_json::json!({}), Duration::from_secs(60));
    let handle = home.create(&routes).await;
    let pushed = turn(&handle, "cmd-1").await;
    assert_eq!(server.received().len(), 6, "一次，加上 5 次换端点");
    assert_eq!(statuses(&pushed).len(), 5);
    let ended = pushed.iter().any(|pushed| {
        matches!(&**pushed, Pushed::Events(events) if events.iter().any(|event| matches!(&event.body, Body::TurnEnded(ended) if ended.reason.as_str() == "error")))
    });
    assert!(ended, "第 6 次出错结束这一轮");
}

#[tokio::test]
async fn errors_that_do_not_fail_over_leave_no_cooling() {
    let mut replies = vec![Reply::error(
        400,
        &[],
        r#"{"error":{"message":"bad request"}}"#,
    )];
    replies.extend(hellos(4));
    let server = Server::start(replies).await;
    let mut home = Home::new();
    home.configs = keyed_configs(&server.base_url, 2);
    let routes = routes(serde_json::json!({}), Duration::from_secs(60));
    let handle = home.create(&routes).await;
    let pushed = turn(&handle, "cmd-1").await;
    assert!(statuses(&pushed).is_empty(), "请求本身有错：不换、不再来");
    assert_eq!(failures(&home, &handle), ["other"]);
    turn(&handle, "cmd-2").await;
    let bearers = bearers(&server);
    assert!(bearers.len() >= 2);
    assert_eq!(bearers[1], bearers[0], "没记冷却：下一轮还是它");
}
