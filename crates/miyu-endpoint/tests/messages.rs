//! 父子之间留言，真核心走一遍（施工 7-7，`docs/blueprint/agents.md` 第六条）：主会话派子代理，子代理派孙代理；孙代理用
//! `to: parent` 问子代理，停下来等；子代理被留言叫醒、答它，留了言的孙代理欠一份回报，子代理这一轮不向上回报；孙代理被
//! 答复叫醒、做完报上来，子代理这才把整件活报给主会话，只报一次。
//!
//! 几个会话同时请求模型，谁先到、留言和回报谁先到都不一定：替身照请求里看到了什么答（[`Brain`]），不照先后排剧本。

mod support;

use std::sync::{Arc, Mutex, PoisonError};

use serde_json::json;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Body, ChildReported, Effect, Event, JobMessaged, Purpose};
use miyu_kernel::id::{JobId, Seq, SessionId};
use miyu_kernel::origin::{By, Model, Session};
use miyu_kernel::request::{Message, Request};
use miyu_session::testkit::{Play, Script};
use miyu_session::{Cancel, ForSession, ModelPort, Models, Reports};
use miyu_tool::Catalog;
use support::{Client, Home, default_resources, until};

/// 照请求里看到了什么答的替身，记下每一次请求。
#[derive(Clone, Default)]
struct Brain(Arc<Mutex<Vec<Request>>>);

impl Brain {
    fn requests(&self) -> Vec<Request> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

/// 请求里人这边的每一段字。
fn said(request: &Request) -> Vec<&str> {
    request
        .messages
        .iter()
        .flat_map(|message| match message {
            Message::User { blocks } => blocks.as_slice(),
            _ => &[],
        })
        .filter_map(|block| match block {
            Block::Text(Text { text }) => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

/// 最后一条是工具结果的话，它的字。
fn last_result(request: &Request) -> Option<&str> {
    match request.messages.last() {
        Some(Message::Tool { blocks, .. }) => blocks.iter().find_map(|block| match block {
            Block::Text(Text { text }) => Some(text.as_str()),
            _ => None,
        }),
        _ => None,
    }
}

/// 调一件工具。
fn calls(name: &str, args: serde_json::Value) -> Play {
    Play::calls(&[(name, &args.to_string())])
}

/// 照请求答：主会话派子代理、等回报；子代理派孙代理、答孙代理的留言、孙代理做完了收尾；孙代理问一句、等答复、做完。
fn decide(request: &Request) -> Play {
    let said = said(request);
    let heard = |text: &str| said.iter().any(|block| block.contains(text));
    let last = last_result(request);
    if heard("派一个去查") {
        return match (last, heard("<subagent-report")) {
            (Some(_), _) => Play::Says("派出去了。"),
            (None, true) => Play::Says("都查完了。"),
            (None, false) => calls("subagent", json!({"description": "查", "prompt": "查 A"})),
        };
    }
    if said.contains(&"查 A") {
        if heard("B 查完了") {
            return Play::Says("A 查完了，B 也在里面。");
        }
        return match last {
            Some(result) if result.starts_with("Started") => Play::Says("等孙代理。"),
            Some(_) => Play::Says("告诉它了，等它做完。"),
            None if heard("<subagent-message") => calls(
                "message_agent",
                json!({"to": "j1.1", "message": "用 a.rs。"}),
            ),
            None => calls("subagent", json!({"description": "查 B", "prompt": "查 B"})),
        };
    }
    if said.contains(&"查 B") {
        if heard("用 a.rs") {
            return Play::Says("B 查完了，改了 a.rs。");
        }
        return match last {
            Some(_) => Play::Says("等答复。"),
            None => calls(
                "message_agent",
                json!({"to": "parent", "message": "要改哪一个文件？"}),
            ),
        };
    }
    panic!("没有哪个会话认这次请求：{said:?}")
}

impl Models for Brain {
    fn port(&self, _: ForSession) -> Arc<dyn ModelPort> {
        Arc::new(self.clone())
    }
}

impl ModelPort for Brain {
    fn model(&self) -> &Model {
        static SCRIPT: std::sync::OnceLock<Script> = std::sync::OnceLock::new();
        SCRIPT.get_or_init(|| Script::new([])).model()
    }

    fn call(&self, seen: Seq, request: Request, reports: Reports, cancel: Cancel) {
        // 起标题的请求（施工 3-8 五补）不带哪一份的原话：不回，一直在路上，不碍这里测的。
        if reports.purpose() == Some(&Purpose::Title) {
            return;
        }
        let play = decide(&request);
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(request.clone());
        Script::new([play]).call(seen, request, reports, cancel);
    }
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

/// 日志里的回报，照先后。
fn reports(log: &[Event]) -> Vec<&ChildReported> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::ChildReported(reported) => Some(reported),
            _ => None,
        })
        .collect()
}

/// 日志里 `by` 是会话 `from` 的那几句话。
fn messages_from<'a>(log: &'a [Event], from: &SessionId) -> Vec<(&'a Event, &'a str)> {
    let by = By::Session(Session { id: from.clone() });
    log.iter()
        .filter(|event| event.by == by)
        .filter_map(|event| match &event.body {
            Body::MessageUser(user) => match user.blocks.as_slice() {
                [Block::Text(Text { text })] => Some((event, text.as_str())),
                other => panic!("留言是一块字：{other:?}"),
            },
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn the_middle_layer_answers_its_subagent_and_reports_the_whole_task_once() {
    let home = Home::new();
    let tools = Catalog::new(miyu_basesystem::tools(&default_resources()).unwrap()).unwrap();
    let brain = Brain::default();
    let mut client = Client::connect(home.core_with_models(Arc::new(brain.clone()), tools));
    client.hello().await;
    let work = home.work.to_string_lossy().into_owned();
    let parent = client.create("c1", &work).await;
    client.say("c2", &parent, "派一个去查").await;

    // 主会话收到子代理的回报，整件活只报一次。
    let done = |log: &[Event]| reports(log).iter().any(|r| r.text.contains("A 查完了"));
    until("主会话收到整件活的回报", || {
        done(&home.log(&parent))
    })
    .await;
    home.until_turns(&parent, 2).await;
    let log = home.log(&parent);
    let child = started_child(&log);
    let texts: Vec<&str> = reports(&log).iter().map(|r| r.text.as_str()).collect();
    assert_eq!(
        texts,
        ["A 查完了，B 也在里面。"],
        "中间一层答完孙代理的那一轮不报，孙代理做完了才把整件活报上去"
    );

    // 子会话：孙代理的留言不带回合编号、by 是孙代理；它答复的那次调用报 `job.messaged`；请求里注明是哪个子代理说的。
    let child_log = home.log(child.as_str());
    let grandchild = started_child(&child_log);
    let asked = messages_from(&child_log, &grandchild);
    let [(message, text)] = asked.as_slice() else {
        panic!("孙代理留了一句言：{asked:?}");
    };
    assert_eq!(*text, "要改哪一个文件？");
    assert_eq!(message.turn, None, "子代理的留言不带回合编号");
    let messaged = child_log.iter().any(|event| {
        matches!(&event.body, Body::ToolResult(result) if result.effects
            == [Effect::JobMessaged(JobMessaged { job: JobId::parse("j1.1").unwrap() })])
    });
    assert!(messaged, "给孙代理的留言报了 job.messaged");
    let tagged = brain.requests().iter().any(|request| {
        said(request).iter().any(|block| {
            block.starts_with("<subagent-message job=\"j1.1\" title=\"查 B\">\n要改哪一个文件？\n")
        })
    });
    assert!(tagged, "子代理的请求里注明了是哪个孙代理说的");
    let finished = reports(&child_log)
        .iter()
        .any(|reported| reported.text == "B 查完了，改了 a.rs。");
    assert!(finished, "孙代理被答复叫醒，做完报上来");

    // 孙会话：子代理的答复原样送来，by 是子代理。
    let grand_log = home.log(grandchild.as_str());
    let answered = messages_from(&grand_log, &child);
    assert!(
        answered.iter().any(|(_, text)| *text == "用 a.rs。"),
        "{answered:?}"
    );
}
