//! `miyu ask --from`（施工 7-10，`docs/blueprint/cli/ask.md`）：在进程里起一个核心，替身模型照请求里人这边的那句话答。和
//! `-s`、`-c` 一起用往那个会话里发，都不写开一个新会话，记成 `harness`、带着名字，她收到的是带标签的那一块；不带 `cwd`，
//! 写了 `--add-dir` 才带 `dirs`；往正忙的会话里发，平常的和 `--from` 都跟住听到它的那一轮；`--from` 按 Ctrl+C、到了
//! `--timeout` 只是不等了，不打断她那一轮。

use std::time::Duration;

use serde_json::json;
use tokio::sync::mpsc;

use crate::support::router::Router;
use crate::support::{Asked, Home, ask_at, plan};
use miyu_cli::{Plan, Target};
use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::SessionId;
use miyu_kernel::origin::By;
use miyu_session::testkit::{Play, Script};

/// 别的 harness 说的那一句。
const SAID: &str = "CI 修好了。";
/// 她收到的那一块：带标签的。
const TAGGED: &str = "<agent-message from=\"claude-code\">\nCI 修好了。\n</agent-message>\n";

/// Claude Code 照 `target` 说那一句。
fn from_claude(target: Target) -> Plan {
    Plan {
        target,
        from: Some("claude-code".to_string()),
        ..plan(SAID)
    }
}

/// Claude Code 的 `by`。
fn claude() -> By {
    serde_json::from_value(json!({"kind": "harness", "name": "claude-code"})).expect("合写法")
}

/// 日志里人这边的话的 `by`，照先后。
fn speakers(log: &[Event]) -> Vec<By> {
    log.iter()
        .filter(|event| matches!(event.body, Body::MessageUser(_)))
        .map(|event| event.by.clone())
        .collect()
}

/// 每一轮开头记的工作目录、加进来的目录，照先后。
fn places(log: &[Event]) -> Vec<(Option<String>, Vec<String>)> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::TurnStarted(started) => Some((started.cwd.clone(), started.dirs.clone())),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn with_session_it_speaks_as_the_harness_and_leaves_the_place_alone() {
    let script = Script::new([
        Play::Says("好。"),
        Play::Says("收到了。"),
        Play::Says("嗯。"),
    ]);
    let home = Home::new(std::sync::Arc::new(script.clone()));
    let kept = Plan {
        dirs: vec!["/kept".to_string()],
        ..plan("hi")
    };
    assert_eq!(home.ask(&kept).await.code, 0);
    let session = home.oneshot().await;
    let elsewhere = Plan {
        cwd: "/elsewhere".to_string(),
        ..from_claude(Target::Session(session.to_string()))
    };
    let asked = home.ask(&elsewhere).await;
    assert_eq!(
        (asked.code, asked.out.as_str()),
        (0, "收到了。\n"),
        "{}",
        asked.err
    );
    let log = home.log(&session);
    assert_eq!(speakers(&log)[1], claude());
    let requests = script.requests();
    assert!(
        requests[1]
            .1
            .messages
            .iter()
            .any(|message| matches!(message,
            miyu_kernel::request::Message::User { blocks } if blocks.iter().any(|block|
                matches!(block, miyu_kernel::block::Block::Text(text) if text.text == TAGGED)))),
        "她收到的是带标签的那一块"
    );
    let places = places(&log);
    assert_eq!(places[0].1, ["/kept"]);
    assert_eq!(
        places[1], places[0],
        "不带 cwd、没写 --add-dir 不带 dirs：会话的工作目录、加进来的目录不变"
    );
    // 写了 --add-dir 的照带。
    let added = Plan {
        dirs: vec!["/extra".to_string()],
        ..from_claude(Target::Session(session.to_string()))
    };
    assert_eq!(home.ask(&added).await.code, 0);
    let places = self::places(&home.log(&session));
    assert_eq!(places[2].0, places[0].0);
    assert_eq!(places[2].1, ["/extra"]);
}

#[tokio::test]
async fn with_continue_and_alone() {
    let script = Script::new([
        Play::Says("好。"),
        Play::Says("收到了。"),
        Play::Says("嗯。"),
    ]);
    let home = Home::new(std::sync::Arc::new(script));
    assert_eq!(home.ask(&plan("hi")).await.code, 0);
    let first = home.oneshot().await;
    assert_eq!(home.ask(&from_claude(Target::Continue)).await.code, 0);
    assert_eq!(
        speakers(&home.log(&first))[1],
        claude(),
        "-c 接着上一次开的会话"
    );
    let asked = home.ask(&from_claude(Target::New)).await;
    assert_eq!(asked.code, 0, "{}", asked.err);
    let newest: SessionId = home.sessions()[0].clone();
    assert_ne!(newest, first, "都不写开一个新会话");
    let log = home.log(&newest);
    let person: By =
        serde_json::from_value(json!({"kind": "person", "account": "admin"})).expect("合写法");
    assert_eq!(log[0].by, person, "会话是本人造的");
    assert_eq!(speakers(&log), [claude()], "第一句记成它发的");
}

/// 往正忙的会话里发：先有人说「跑个长活」，那一次请求停在闸上；另一个 `miyu ask -s` 说的这一句落了盘再放行。
async fn into_a_busy_session(
    router: Router,
    gate: crate::support::router::Gate,
    second: Plan,
) -> String {
    let home = Home::new(std::sync::Arc::new(router));
    let (_press, presses) = mpsc::channel(1);
    let long = plan("跑个长活");
    let first = ask_at(&home.root, &long, presses);
    let second = async {
        let session = home.until_a_turn_starts().await;
        let second = Plan {
            target: Target::Session(session.to_string()),
            ..second
        };
        let (_press, presses) = mpsc::channel(1);
        let asking = ask_at(&home.root, &second, presses);
        let opening = async {
            crate::support::within("这一句落了盘", async {
                while speakers(&home.log(&session)).len() < 2 {
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            })
            .await;
            gate.open();
        };
        let (asked, ()) = tokio::join!(asking, opening);
        asked
    };
    let (first, second) = tokio::join!(first, second);
    assert_eq!(first.code, 0, "{}", first.err);
    assert_eq!(second.code, 0, "{}", second.err);
    second.out
}

#[tokio::test]
async fn a_word_into_a_busy_session_follows_the_turn_that_hears_it() {
    // 人的话排进在进行的那一轮：闸放开，她调一次（不存在的）工具，下一步听到这一句、答它，这一轮就是它的。
    let mut router = Router::new(SAID, [Play::Says("收到了。")]);
    let gate = router.gated("跑个长活", [Play::calls(&[("nothing", "{}")])]);
    let out = into_a_busy_session(router, gate, plan(SAID)).await;
    assert_eq!(out, "收到了。\n");
}

#[tokio::test]
async fn a_harness_word_into_a_busy_session_follows_the_next_turn_when_unheard() {
    // 别的 harness 的话：闸放开时那一次请求没看到它，她说完这一轮就结束；同一批由它接着开的下一轮才是它的。
    let mut router = Router::new(TAGGED, [Play::Says("收到了。")]);
    let gate = router.gated("跑个长活", [Play::Says("长活做完了。")]);
    let out = into_a_busy_session(router, gate, from_claude(Target::New)).await;
    assert_eq!(out, "长活做完了。\n收到了。\n", "接上以后的都印");
}

/// 别的 harness 往正忙的会话里发，它不等了：`timeout` 是写了 `--timeout`，没写的按一次 Ctrl+C（这一句落了盘以后）。那一轮停
/// 在闸上，它走了才放行。交回它看到的，和先说「跑个长活」的那个 `miyu ask` 看到的。
async fn leaving_a_busy_session(timeout: Option<Duration>) -> (Asked, Asked, Vec<Event>) {
    let mut router = Router::new(TAGGED, [Play::Says("收到了。")]);
    let gate = router.gated("跑个长活", [Play::Says("长活做完了。")]);
    let home = Home::new(std::sync::Arc::new(router));
    let (_press, presses) = mpsc::channel(1);
    let long = plan("跑个长活");
    let first = ask_at(&home.root, &long, presses);
    let second = async {
        let session = home.until_a_turn_starts().await;
        let second = Plan {
            timeout,
            ..from_claude(Target::Session(session.to_string()))
        };
        let (press, presses) = mpsc::channel(1);
        let asking = ask_at(&home.root, &second, presses);
        let pressing = async {
            if timeout.is_some() {
                return;
            }
            crate::support::within("这一句落了盘", async {
                while speakers(&home.log(&session)).len() < 2 {
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            })
            .await;
            press.send(()).await.expect("还在等 Ctrl+C");
        };
        let (asked, ()) = tokio::join!(asking, pressing);
        // 它的连接断了，它发过的都办完了（发了打断的，那一轮这时已经打断了），再放行。
        home.until_connections(1).await;
        gate.open();
        (asked, session)
    };
    let (first, (second, session)) = tokio::join!(first, second);
    home.until_ended(&session, 2).await;
    (first, second, home.log(&session))
}

/// 它走了，她那一轮没被打断：先说话的那个照常答完，别的 harness 的话接着开了下一轮。
fn not_interrupted(first: &Asked, log: &[Event]) {
    assert_eq!(
        (first.code, first.out.as_str()),
        (0, "长活做完了。\n"),
        "{}",
        first.err
    );
    let reasons: Vec<String> = log
        .iter()
        .filter_map(|event| match &event.body {
            Body::TurnEnded(ended) => Some(ended.reason.as_str().to_string()),
            _ => None,
        })
        .collect();
    assert_eq!(reasons, ["completed", "completed"], "没有打断：{log:#?}");
}

#[tokio::test]
async fn ctrl_c_from_another_harness_only_stops_waiting() {
    let (first, second, log) = leaving_a_busy_session(None).await;
    assert_eq!(second.code, 3, "{}", second.err);
    assert!(
        second.err.ends_with("· 不等了，她那一轮还在接着跑\n"),
        "{}",
        second.err
    );
    not_interrupted(&first, &log);
}

#[tokio::test]
async fn the_timeout_of_another_harness_only_stops_waiting() {
    let (first, second, log) = leaving_a_busy_session(Some(Duration::from_secs(1))).await;
    assert_eq!(second.code, 3, "{}", second.err);
    assert!(
        second.err.ends_with("· 等到时间了，她那一轮还在接着跑\n"),
        "{}",
        second.err
    );
    not_interrupted(&first, &log);
}
