//! 发给别的会话，真核心走一遍（施工 C-5，`docs/blueprint/cross-session.md` 第三条、第五条）：两个主会话 A、B，同一个
//! 属主。A 先列会话找到 B，再给它发一句；B 闲着被叫醒，回一句；A 又被叫醒，接着回，一来一回。每一句都要对方开一轮才有
//! 回话，照第五条第 6 款，连续发到第 6 句（限速 `peers.burst` 是 5）就会被拒，链自然断掉：A 比 B 先发，发得也比 B 多一句，
//! 第 6 句轮到 A 被拒，B 永远收不到第 6 句、停在它的第 5 轮。

use std::sync::{Arc, Mutex, PoisonError};

use serde_json::json;

use crate::support::{Client, Home, default_resources, until};
use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Body, Event, Purpose};
use miyu_kernel::id::{Seq, SessionId};
use miyu_kernel::origin::Model;
use miyu_kernel::request::{Message, Request};
use miyu_session::testkit::{Play, Script};
use miyu_session::{Cancel, ForSession, ModelPort, Models, Reports};
use miyu_tool::Catalog;

/// 照请求里看到了什么答的替身，记下每一次请求。
#[derive(Clone, Default)]
struct Brain(Arc<Mutex<Vec<Request>>>);

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
        // 起标题的请求（施工 3-8 五补）不带哪一份的原话：不回，一直在路上，不碍这里测的。
        if reports.purpose() == Some(&Purpose::Title) {
            return;
        }
        let play = decide(&request);
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(request.clone());
        Script::new([play]).call(seen, request, config, reports, cancel);
    }
}

/// 请求里人这边的每一段字（含别的会话发来的话，施工 C-2：它也渲染进 `Message::User`）。
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

/// 别的会话发来的话里带的数字：`<session-message from="{短编号}">\n{数字}\n</session-message>`。会话的上下文是累计的，
/// 旧的一句也还在里面，要拿最后（最新）一句，不能拿第一条碰到的。
fn tagged_number<'a>(texts: &[&'a str]) -> Option<(&'a str, u32)> {
    texts.iter().rev().find_map(|block| {
        let rest = block.strip_prefix("<session-message from=\"")?;
        let (from, rest) = rest.split_once("\">\n")?;
        let (number, _) = rest.split_once("\n</session-message>")?;
        Some((from, number.trim().parse().ok()?))
    })
}

/// 照请求答：A 先列会话、发第一句；谁收到带数字的话就回给对方「数字加一」；工具结果说送到了就收尾，说别的（被拒）也
/// 收尾、不再发。先认带数字的那句（累计的上下文里「开始」一直都在，得先认更新的那一句，不然每次都当成第一轮）。
fn decide(request: &Request) -> Play {
    if let Some(result) = last_result(request) {
        if result.starts_with("You are session") {
            // A 列完了，找到 B：这个测试直接拿已知的编号发，不解析列表的字（列表本身另有单测守着）。
            return calls(
                "send_message",
                json!({"to": b_id().to_string(), "message": "1"}),
            );
        }
        return match result.starts_with("Message sent to") {
            true => Play::Says("好。"),
            false => Play::Says("收到拒绝，不再发了。"),
        };
    }
    let texts = said(request);
    if let Some((from, number)) = tagged_number(&texts) {
        let to = match from == a_id().short() {
            true => a_id(),
            false => b_id(),
        };
        return calls(
            "send_message",
            json!({"to": to.to_string(), "message": (number + 1).to_string()}),
        );
    }
    if texts.iter().any(|block| block.contains("开始")) {
        return calls("sessions", json!({}));
    }
    panic!("没认出来：{texts:?}")
}

/// 两个会话的编号，开工前先造好、记在这两个函数里（`OnceLock`）：`decide` 是自由函数，拿不到测试里的局部变量。
fn a_id() -> SessionId {
    IDS.get().expect("先造好会话").0.clone()
}
fn b_id() -> SessionId {
    IDS.get().expect("先造好会话").1.clone()
}
static IDS: std::sync::OnceLock<(SessionId, SessionId)> = std::sync::OnceLock::new();

/// 日志里带着给她看的字是 `text` 的工具结果有几条。
fn count_results(log: &[Event], text: &str) -> usize {
    log.iter()
        .filter(|event| match &event.body {
            Body::ToolResult(result) => result
                .blocks
                .iter()
                .any(|block| matches!(block, Block::Text(Text { text: t }) if t.contains(text))),
            _ => false,
        })
        .count()
}

#[tokio::test]
async fn two_sessions_message_back_and_forth_until_the_burst_limit_stops_them() {
    let home = Home::new();
    let tools = Catalog::new(miyu_basesystem::tools(&default_resources()).unwrap()).unwrap();
    let brain = Brain::default();
    let mut client = Client::connect(home.core_with_models(Arc::new(brain.clone()), tools));
    client.hello().await;
    let work = home.work.to_string_lossy().into_owned();
    let a = client.create("c1", &work).await;
    let b = client.create("c2", &work).await;
    IDS.set((
        SessionId::parse(&a).expect("合写法"),
        SessionId::parse(&b).expect("合写法"),
    ))
    .expect("只设一次");

    client.say("c3", &a, "开始").await;
    until("A 发到第 6 句被挡住", || {
        count_results(&home.log(&a), "Too many messages") == 1
    })
    .await;

    let a_log = home.log(&a);
    let b_log = home.log(&b);
    assert_eq!(
        count_results(&a_log, "You are session"),
        1,
        "A 列过一次会话"
    );
    assert_eq!(
        count_results(&a_log, "Message sent to"),
        5,
        "A 送到了 5 句：1、3、5、7、9"
    );
    assert_eq!(
        count_results(&a_log, "Too many messages"),
        1,
        "第 6 句（11）被拒"
    );
    assert_eq!(
        count_results(&b_log, "Message sent to"),
        5,
        "B 送到了 5 句：2、4、6、8、10"
    );
    assert_eq!(
        count_results(&b_log, "Too many messages"),
        0,
        "B 没到上限：A 的第 6 句没发出去，B 没再被叫醒"
    );
}
