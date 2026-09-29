//! 子会话回报，真核心走一遍（施工 7-6，`docs/blueprint/agents.md` 第二条）：主会话派子代理，子代理再派孙代理；替身模型在
//! 各个会话里答话，回报一层一层上来：孙代理报给子代理、叫醒它，子代理这才把整件事报给主会话、叫醒她。
//!
//! 几个会话同时请求模型，谁先到不一定：替身照请求里人这边的那句交代分给各自的剧本。

mod support;

use std::sync::Arc;

use serde_json::json;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Body, ChildReason, ChildReported, Effect, Event};
use miyu_kernel::id::{JobId, Seq, SessionId};
use miyu_kernel::origin::{By, Model, Session};
use miyu_kernel::request::{Message, Request};
use miyu_session::testkit::{Play, Script};
use miyu_session::{Cancel, ForSession, ModelPort, Models, Reports};
use miyu_tool::Catalog;
use support::{Client, Home, default_resources};

/// 各个会话的剧本：请求里人这边有哪一句，就照哪一份回。
#[derive(Clone)]
struct Router(Arc<Vec<(&'static str, Script)>>);

impl Router {
    fn script(&self, request: &Request) -> &Script {
        let said = |key: &str| {
            request.messages.iter().any(|message| match message {
                Message::User { blocks, .. } => blocks.contains(&Block::Text(Text {
                    text: key.to_string(),
                })),
                _ => false,
            })
        };
        self.0
            .iter()
            .find(|(key, _)| said(key))
            .map(|(_, script)| script)
            .unwrap_or_else(|| panic!("没有哪份剧本认这次请求"))
    }
}

impl Models for Router {
    fn port(&self, _: ForSession) -> Arc<dyn ModelPort> {
        Arc::new(self.clone())
    }
}

impl ModelPort for Router {
    fn model(&self) -> &Model {
        self.0[0].1.model()
    }

    fn call(&self, seen: Seq, request: Request, reports: Reports, cancel: Cancel) {
        let script = self.script(&request).clone();
        script.call(seen, request, reports, cancel);
    }
}

/// 派一个子代理，交代是 `prompt`。
fn agent(prompt: &str) -> String {
    json!({"description": "查", "prompt": prompt}).to_string()
}

/// 日志里 `job.started` 记着的子会话。
fn started_child(log: &[Event]) -> SessionId {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::ToolResult(result) => Some(&result.effects),
            _ => None,
        })
        .flatten()
        .find_map(|effect| match effect {
            Effect::JobStarted(started) => started.session.clone(),
            _ => None,
        })
        .expect("派出去了一个子会话")
}

/// 日志里最后开的那一轮。
fn last_turn(log: &[Event]) -> Seq {
    log.iter()
        .rev()
        .find(|event| matches!(event.body, Body::TurnStarted(_)))
        .map(|event| event.seq)
        .expect("开过一轮")
}

/// 日志里唯一的一条回报，和紧跟着它开的那一轮。
fn the_report(log: &[Event]) -> (&Event, &ChildReported) {
    let mut reports = log.iter().filter_map(|event| match &event.body {
        Body::ChildReported(reported) => Some((event, reported)),
        _ => None,
    });
    let found = reports.next().expect("有一条回报");
    assert!(reports.next().is_none(), "只报了一次");
    let (event, _) = found;
    let opened = log.iter().any(|later| {
        matches!(&later.body, Body::TurnStarted(started) if started.trigger == Some(event.seq))
    });
    assert!(opened, "回报开了一轮");
    found
}

#[tokio::test]
async fn reports_come_up_one_layer_at_a_time() {
    let home = Home::new();
    let tools = Catalog::new(miyu_basesystem::tools(&default_resources()).unwrap()).unwrap();
    let main = Script::new([
        Play::calls(&[("agent", &agent("查 A"))]),
        Play::Says("派出去了。"),
        Play::Says("都查完了。"),
    ]);
    let child = Script::new([
        Play::calls(&[("agent", &agent("查 B"))]),
        Play::Says("等孙代理。"),
        Play::Says("A 查完了，B 也在里面。"),
    ]);
    let grandchild = Script::new([Play::Says("B 查完了。")]);
    let router = Router(Arc::new(vec![
        ("派一个去查", main),
        ("查 A", child),
        ("查 B", grandchild),
    ]));
    let mut client = Client::connect(home.core_with_models(Arc::new(router), tools));
    client.hello().await;
    let work = home.work.to_string_lossy().into_owned();
    let parent = client.create("c1", &work).await;
    client.say("c2", &parent, "派一个去查").await;

    // 主会话：派出去的那一轮，和子代理的回报开的那一轮。
    home.until_turns(&parent, 2).await;
    let log = home.log(&parent);
    let child = started_child(&log);
    let (event, reported) = the_report(&log);
    assert_eq!(event.by, By::Session(Session { id: child.clone() }));
    let woken = last_turn(&home.log(child.as_str()));
    assert_eq!(
        event.cause.as_ref().map(|cause| cause.as_str().to_string()),
        Some(format!("{child}/report/{woken}")),
        "命令编号照子会话报的那一轮：孙代理的回报叫醒的那一轮"
    );
    assert_eq!(
        *reported,
        ChildReported {
            job: JobId::new(1).unwrap(),
            session: child.clone(),
            reason: ChildReason::Done,
            text: "A 查完了，B 也在里面。".to_string(),
            truncated: false,
            person: false,
        },
        "孙代理报完、它被叫醒的那一轮结束了，才报；报的是那一轮最后说的"
    );

    // 子会话：派孙代理的那一轮不报，孙代理的回报叫醒它。
    let log = home.log(child.as_str());
    let grandchild = started_child(&log);
    let (event, reported) = the_report(&log);
    assert_eq!(
        event.by,
        By::Session(Session {
            id: grandchild.clone()
        })
    );
    assert_eq!(reported.text, "B 查完了。");
    assert_eq!(reported.session, grandchild);
    let Body::SessionCreated(created) = &home.log(grandchild.as_str())[0].body else {
        panic!("第 1 条应该是造会话");
    };
    assert_eq!(created.depth, Some(2));
}

/// 子代理做到一半，核心有计划地重启了：再起来以后没人叫它；父会话一载入就把它叫起来，它接着干完、报上来。
#[tokio::test]
async fn a_parent_loaded_after_a_restart_wakes_its_child_to_finish_and_report() {
    let home = Home::new();
    let tools = || Catalog::new(miyu_basesystem::tools(&default_resources()).unwrap()).unwrap();
    let before = Router(Arc::new(vec![
        (
            "派一个去查",
            Script::new([
                Play::calls(&[("agent", &agent("查 A"))]),
                Play::Says("派出去了。"),
            ]),
        ),
        ("查 A", Script::new([Play::Holds])),
    ]));
    let first = home.core_with_models(Arc::new(before.clone()), tools());
    let mut client = Client::connect(first.clone());
    client.hello().await;
    let work = home.work.to_string_lossy().into_owned();
    let parent = client.create("c1", &work).await;
    client.say("c2", &parent, "派一个去查").await;
    home.until_turns(&parent, 1).await;
    let child = started_child(&home.log(&parent));
    // 子会话的请求停在路上。
    while before.0[1].1.requests().is_empty() {
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    first.stop_sessions().await;
    drop(client);
    home.until_turns(child.as_str(), 1).await;

    let after = Router(Arc::new(vec![
        ("派一个去查", Script::new([Play::Says("收到了。")])),
        ("查 A", Script::new([Play::Says("A 查完了。")])),
    ]));
    let mut client = Client::connect(home.core_with_models(Arc::new(after), tools()));
    client.hello().await;
    client.subscribe("w1", &parent).await;
    home.until_turns(&parent, 2).await;
    let log = home.log(&parent);
    let (_, reported) = the_report(&log);
    assert_eq!(reported.text, "A 查完了。", "接着干完的那一轮报上来");
    assert_eq!(reported.reason, ChildReason::Done);
}
