//! 「空了告诉我」，真核心走一遍（施工 C-6，`docs/blueprint/cross-session.md` 第六条，2026-10-01 项目主人定改了
//! 「已经空着」那一条）：两个主会话 A、B，同一个属主。
//!
//! - 只订不发：B 闲着，A 订了它，B 不开轮；B 这时已经空着，通知不当场到；B 自己又做完一轮，A 才被叫醒，看到
//!   B 新那一轮的第一行。
//! - 带话的先送话再等：A 给 B 发一句、同时订，B 被那句话叫醒、做完，A 再被通知叫醒，通知带着 B 那一轮最后说的话。
//! - 核心重启：B 正忙的时候 A 订了它，核心停了；换一份核心，A 一载入就照日志再订，B 跟着载入、接着把被打断的那一轮做完，
//!   A 照样等到。B 被重启打断不算空下来：停的时候不发。
//! - 被等的会话删了：换一份核心以后先删 B，A 载入再订时会话表找不到它，记 `gone`、不叫醒她。
//!
//! 模型是照请求答的替身：每一次照请求里最新的那一段字决定说什么、调什么。

mod support;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use serde_json::json;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Body, Event, IdleReason};
use miyu_kernel::id::Seq;
use miyu_kernel::origin::Model;
use miyu_kernel::request::{Message, Request};
use miyu_session::testkit::{Play, Script};
use miyu_session::{Cancel, ForSession, ModelPort, Models, Reports};
use miyu_tool::Catalog;
use support::{Client, Home, default_resources, until};

/// 照请求里最新的那一段字答的替身。`held` 是「慢慢做」停住了。
#[derive(Clone, Default)]
struct Brain {
    held: Arc<AtomicBool>,
    requests: Arc<Mutex<Vec<Request>>>,
}

impl Models for Brain {
    fn port(&self, _: ForSession) -> Arc<dyn ModelPort> {
        Arc::new(self.clone())
    }
}

impl ModelPort for Brain {
    fn model(&self) -> Model {
        static SCRIPT: std::sync::OnceLock<Script> = std::sync::OnceLock::new();
        SCRIPT.get_or_init(|| Script::new([])).model()
    }

    fn call(
        &self,
        seen: Seq,
        request: Request,
        config: &miyu_session::TurnConfig,
        reports: Reports,
        cancel: Cancel,
    ) {
        // 起标题、回顾这类辅助请求不回：一直在路上，不碍这里测的。
        if reports.purpose().is_some() {
            return;
        }
        let play = self.decide(&request);
        self.requests
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(request.clone());
        Script::new([play]).call(seen, request, config, reports, cancel);
    }
}

impl Brain {
    /// 照请求答。最后一条是工具结果的，收尾；不然照人这边最新的那一段字。
    fn decide(&self, request: &Request) -> Play {
        if let Some(Message::Tool { .. }) = request.messages.last() {
            return Play::Says("等它。");
        }
        let latest = latest(request);
        if latest.starts_with("<session-idle") {
            return Play::Says("它做完了。");
        }
        if latest.starts_with("<session-message") {
            return Play::Says("测试全过了。\n细节略。");
        }
        if let Some(to) = latest.strip_prefix("订 ") {
            return send(json!({"to": to.trim(), "notify_when_idle": true}));
        }
        if let Some(to) = latest.strip_prefix("让它跑测试 ") {
            let args = json!({"to": to.trim(), "message": "跑一遍测试", "notify_when_idle": true});
            return send(args);
        }
        if latest.contains("做点事") {
            return Play::Says("做完了：一切正常。");
        }
        // 被重启打断的那一轮接着干：请求最后是「重启打断了这一轮」那一句。
        if latest.contains("reason=\"restarted\"") {
            return Play::Says("慢慢做完了。");
        }
        if latest.contains("慢慢做") {
            self.held.store(true, Ordering::Release);
            return Play::Holds;
        }
        panic!("没认出来：{latest:?}")
    }
}

/// 人这边最新的那一段字：最后一条 user 的最后一块字。
fn latest(request: &Request) -> &str {
    request
        .messages
        .iter()
        .rev()
        .find_map(|message| match message {
            Message::User { blocks } => blocks.iter().rev().find_map(|block| match block {
                Block::Text(Text { text }) => Some(text.as_str()),
                _ => None,
            }),
            _ => None,
        })
        .unwrap_or_default()
}

/// 调一次 `send_message`。
fn send(args: serde_json::Value) -> Play {
    Play::calls(&[("send_message", &args.to_string())])
}

/// 真的基础系统。
fn tools() -> Catalog {
    Catalog::new(miyu_basesystem::tools(&default_resources()).expect("读得出")).expect("合写法")
}

/// 日志里的 `peer.idle`：原因、带的那一行。
fn notices(log: &[Event]) -> Vec<(IdleReason, Option<String>)> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::PeerIdle(idle) => Some((idle.reason.clone(), idle.status.clone())),
            _ => None,
        })
        .collect()
}

/// 等会话表说 `session` 不忙了（`session.list` 的 `busy`，最多十秒）。
async fn until_idle(client: &mut Client, session: &str) {
    for n in 0..2000 {
        let reply = client
            .call(&format!("idle-{n}"), "session.list", serde_json::json!({}))
            .await;
        let busy = reply["result"]["sessions"]
            .as_array()
            .expect("有会话")
            .iter()
            .any(|item| item["session"] == session && item["busy"] == true);
        if !busy {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    panic!("十秒内 {session} 没空下来");
}

/// 日志里开了几轮。
fn turns(log: &[Event]) -> usize {
    log.iter()
        .filter(|event| matches!(event.body, Body::TurnStarted(_)))
        .count()
}

/// 由 `peer.idle` 开的那一轮有没有：回合开头的触发是一条 `peer.idle`。
fn woken_by_a_notice(log: &[Event]) -> bool {
    log.iter().any(|event| match &event.body {
        Body::TurnStarted(started) => started.trigger.is_some_and(|trigger| {
            log.iter()
                .any(|event| event.seq == trigger && matches!(event.body, Body::PeerIdle(_)))
        }),
        _ => false,
    })
}

#[tokio::test]
async fn watching_alone_opens_no_turn_there_and_waits_for_its_next_turn_there() {
    let home = Home::new();
    let brain = Brain::default();
    let mut client = Client::connect(home.core_with_models(Arc::new(brain.clone()), tools()));
    client.hello().await;
    let work = home.work.to_string_lossy().into_owned();
    let a = client.create("c1", &work).await;
    let b = client.create("c2", &work).await;
    client.say("c3", &b, "做点事").await;
    home.until_turns(&b, 1).await;
    // 日志里有了 `turn.ended` 不等于会话已经空了：落盘以后还要更新派生的索引，才标成空闲。照会话表说的等它空下来。
    until_idle(&mut client, &b).await;
    client.say("c4", &a, &format!("订 {b}")).await;
    home.until_turns(&a, 1).await;
    assert!(
        notices(&home.log(&a)).is_empty(),
        "B 订进来的时候已经空着：不当场发（2026-10-01 改）"
    );
    // B 自己再做一轮，A 才该被叫醒：带的是这新一轮的第一行，不是订进来时那一轮的旧回答。
    client.say("c5", &b, "做点事").await;
    home.until_turns(&b, 2).await;
    until("A 被通知叫醒、做完", || {
        woken_by_a_notice(&home.log(&a)) && turns(&home.log(&a)) == 2 && {
            let log = home.log(&a);
            log.last()
                .is_some_and(|event| matches!(event.body, Body::TurnEnded(_)))
        }
    })
    .await;
    assert_eq!(
        notices(&home.log(&a)),
        [(IdleReason::Idle, Some("做完了：一切正常。".to_string()))],
        "B 新那一轮的第一行"
    );
    assert_eq!(
        turns(&home.log(&b)),
        2,
        "只订不发：B 没为订开轮，这是它自己做的第二轮"
    );
    let short = &b[b.len() - 8..];
    let seen = brain
        .requests
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .any(|request| {
            latest(request)
                == format!(
                    "<session-idle session=\"{short}\" reason=\"idle\">\n做完了：一切正常。\n</session-idle>\n"
                )
        });
    assert!(seen, "她看到的通知是那一块带标签的事实");
}

#[tokio::test]
async fn a_message_goes_first_and_the_notice_comes_after_that_turn() {
    let home = Home::new();
    let brain = Brain::default();
    let mut client = Client::connect(home.core_with_models(Arc::new(brain.clone()), tools()));
    client.hello().await;
    let work = home.work.to_string_lossy().into_owned();
    let a = client.create("c1", &work).await;
    let b = client.create("c2", &work).await;
    client.say("c3", &a, &format!("让它跑测试 {b}")).await;
    until("A 被通知叫醒", || woken_by_a_notice(&home.log(&a))).await;
    assert_eq!(
        notices(&home.log(&a)),
        [(IdleReason::Idle, Some("测试全过了。".to_string()))],
        "通知带着 B 被那句话叫醒的那一轮的第一行"
    );
    let b_log = home.log(&b);
    assert_eq!(turns(&b_log), 1, "B 被那句话叫醒了一轮");
    assert!(
        b_log
            .iter()
            .any(|event| matches!(event.body, Body::MessageUser(_)) && event.turn.is_none()),
        "先送到了那句话"
    );
}

#[tokio::test]
async fn after_a_restart_the_waiting_session_watches_again_and_still_hears_back() {
    let home = Home::new();
    let brain = Brain::default();
    let first = home.core_with_models(Arc::new(brain.clone()), tools());
    let mut client = Client::connect(first.clone());
    client.hello().await;
    let work = home.work.to_string_lossy().into_owned();
    let a = client.create("c1", &work).await;
    let b = client.create("c2", &work).await;
    client.say("c3", &b, "慢慢做").await;
    until("B 停在半路", || brain.held.load(Ordering::Acquire)).await;
    client.say("c4", &a, &format!("订 {b}")).await;
    home.until_turns(&a, 1).await;
    assert!(notices(&home.log(&a)).is_empty(), "B 正忙，还没通知");
    first.stop_sessions().await;
    drop(client);
    assert!(
        notices(&home.log(&a)).is_empty(),
        "B 被重启打断不算空下来：停的时候不发"
    );
    // 换一份核心：A 订阅就载入，载入以后照日志再订，B 跟着载入、接着做完被打断的那一轮。
    let mut client = Client::connect(home.core_with_models(Arc::new(brain.clone()), tools()));
    client.hello().await;
    client.subscribe("s1", &a).await;
    until("A 照样等到", || woken_by_a_notice(&home.log(&a))).await;
    assert_eq!(
        notices(&home.log(&a)),
        [(IdleReason::Idle, Some("慢慢做完了。".to_string()))]
    );
}

#[tokio::test]
async fn a_deleted_session_is_found_gone_when_she_watches_it_again() {
    let home = Home::new();
    let brain = Brain::default();
    let first = home.core_with_models(Arc::new(brain.clone()), tools());
    let mut client = Client::connect(first.clone());
    client.hello().await;
    let work = home.work.to_string_lossy().into_owned();
    let a = client.create("c1", &work).await;
    let b = client.create("c2", &work).await;
    client.say("c3", &b, "慢慢做").await;
    until("B 停在半路", || brain.held.load(Ordering::Acquire)).await;
    client.say("c4", &a, &format!("订 {b}")).await;
    home.until_turns(&a, 1).await;
    first.stop_sessions().await;
    drop(client);
    // 换一份核心，先删掉 B，再让 A 载入：A 照日志再订，会话表找不到 B，记 `gone`、不叫醒她。
    let mut client = Client::connect(home.core_with_models(Arc::new(brain.clone()), tools()));
    client.hello().await;
    // 删的时候 B 跟着载入、接着做完被打断的那一轮；做完之前删不了（`turn_running`），删到成为止。
    let mut tries = 0;
    loop {
        tries += 1;
        let deleted = client
            .call(
                &format!("d{tries}"),
                "session.delete",
                json!({"session": b}),
            )
            .await;
        if deleted.get("error").is_none() {
            break;
        }
        assert!(tries < 500, "删不掉 B：{deleted}");
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    client.subscribe("s1", &a).await;
    until("A 记下不在了", || !notices(&home.log(&a)).is_empty()).await;
    assert_eq!(notices(&home.log(&a)), [(IdleReason::Gone, None)]);
    assert_eq!(turns(&home.log(&a)), 1, "不在了只记下，不叫醒");
}
