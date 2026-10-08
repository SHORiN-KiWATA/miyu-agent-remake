//! `send_message`（`docs/blueprint/tools/send_message.md`，施工 7-7、C-5）：只声明 `to`、`message`；`parent` 发给父会话，任务
//! 编号发给自己派的子代理，别的当会话编号认（和 `sessions`、`history` 同一个认法），经端口原样送出去；送到了说一句，是没人
//! 看着的一次性会话只说存下了；给子代理的报 `job.messaged`；别的都拒，每一种说清为什么：没有父、不是她派的（写法都不对的
//! 端口不问）、被停掉了、送不到、没有端口、找不到会话、撞了、是她自己、这个会话不能发给别的会话、太长、限速、一模一样的
//! （不算出错）、对方没看的太多；参数不对的照共用的那一句。以前的名字 `message_agent` 还认得出。给人看的说法两种语言都
//! 换得出字。施工 C-6 多一格 `notify_when_idle`、`message` 可以不写（`send_message/watch.rs`）。

/// 当会话编号认（施工 C-5）：找到的、找不到的、撞了的、是她自己的，每种说法；这个文件超过行数上限，拆进这里
/// （照 `spawn/renamed.rs` 的先例，`#[path]` 一样要写：这个文件是 crate 根，`mod` 默认只找同目录的平级文件）。
#[path = "send_message/by_session_id.rs"]
mod by_session_id;

/// 空了告诉我（施工 C-6）：拆进这里，理由同上。
#[path = "send_message/watch.rs"]
mod watch;

use std::sync::{Arc, Mutex, PoisonError};

use serde_json::json;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::JobMessaged;
use miyu_kernel::id::JobId;
use miyu_kernel::tool::Access;
use miyu_tool::{Delivered, Done, Effect, MessagePort, NotSent, Recipient, Sending};

use crate::support::{Site, check, human, readable, said, tool};

/// 她自己：短编号 `22334455`（和 `sessions.rs` 共用）。
const THIS: &str = "0192f3a0-1111-7abc-8def-001122334455";
/// 别的会话：短编号 `9f03b21c`（和 `sessions.rs` 共用）。
const OTHER: &str = "0192f3a0-2222-7abc-8def-00119f03b21c";

/// 假的端口：记下交给它的每一次发给谁、说了什么，照 `answer` 回。
struct Port {
    answer: Result<Delivered, NotSent>,
    asked: Mutex<Vec<(Recipient, String)>>,
}

impl Port {
    fn new(answer: Result<Delivered, NotSent>) -> Arc<Port> {
        Arc::new(Port {
            answer,
            asked: Mutex::new(Vec::new()),
        })
    }

    fn asked(&self) -> Vec<(Recipient, String)> {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl MessagePort for Port {
    fn send<'a>(&'a self, to: Recipient, message: &'a str) -> Sending<'a> {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((to, message.to_string()));
        let answer = self.answer;
        Box::pin(async move { answer })
    }
}

/// 交回的那一段字。
fn text(done: &Done) -> &str {
    match done.blocks.as_slice() {
        [Block::Text(Text { text })] => text,
        other => panic!("只有一段字：{other:?}"),
    }
}

/// 发给 `to`。
fn to(to: &str) -> serde_json::Value {
    json!({"to": to, "message": "Which file should I change?\nOnly one."})
}

#[test]
fn it_declares_the_recipient_the_message_and_the_watch() {
    let tool = tool("send_message");
    let spec = tool.spec();
    assert_eq!(spec.name, "send_message");
    // 留言什么都不改：只读开着也发得出去，给几个会话留言连着的一起发。
    assert_eq!(spec.access, Access::Read);
    let parameters: serde_json::Value = serde_json::from_str(spec.parameters.get()).unwrap();
    let names: Vec<&String> = parameters["properties"]
        .as_object()
        .unwrap()
        .keys()
        .collect();
    assert_eq!(names, ["message", "notify_when_idle", "to"]);
    assert_eq!(
        parameters["required"],
        json!(["to"]),
        "只订不发的可以不写 message（施工 C-6）"
    );
    assert_eq!(
        parameters["properties"]["notify_when_idle"]["type"],
        "boolean"
    );
    assert!(
        spec.description
            .contains("Send only what they need to know now"),
        "只发对方现在就得知道的那一句留着：{}",
        spec.description
    );
    assert!(
        spec.description
            .contains("or to another of your sessions by its id"),
        "能发给别的会话：{}",
        spec.description
    );
    // 施工 C-5 从 `message_agent` 改名：以前的名字照样认得出。
    assert_eq!(tool.formerly(), ["message_agent"]);
}

#[tokio::test]
async fn it_sends_to_a_subagent_or_the_parent_as_it_is() {
    let site = Site::new();
    let port = Port::new(Ok(Delivered::Sent));
    let done = site
        .done_with_messages("send_message", to("j2"), Some(port.clone()))
        .await;
    assert!(!done.error);
    assert_eq!(text(&done), "Message sent to j2.\n");
    assert_eq!(
        done.effects,
        [Effect::JobMessaged(JobMessaged {
            job: JobId::new(2).unwrap()
        })],
        "给子代理留了言：它欠一份回报"
    );
    let done = site
        .done_with_messages("send_message", to("parent"), Some(port.clone()))
        .await;
    assert_eq!(text(&done), "Message sent to parent.\n");
    assert!(done.effects.is_empty(), "发给父的不报效果：父会话不欠谁的");
    let message = "Which file should I change?\nOnly one.".to_string();
    assert_eq!(
        port.asked(),
        [
            (Recipient::Child(JobId::new(2).unwrap()), message.clone()),
            (Recipient::Parent, message)
        ],
        "发给谁照 to 认，话原样交出去"
    );
    // 子会话派的带着前缀（施工 7-1 补）：几段的编号照样认。
    let port = Port::new(Ok(Delivered::Sent));
    let done = site
        .done_with_messages("send_message", to("j2.1"), Some(port.clone()))
        .await;
    assert_eq!(text(&done), "Message sent to j2.1.\n");
    let [(recipient, _)] = port.asked().try_into().expect("问过一次");
    assert_eq!(recipient, Recipient::Child(JobId::parse("j2.1").unwrap()));
}

#[tokio::test]
async fn a_held_message_is_not_an_error() {
    let site = Site::new();
    let port = Port::new(Ok(Delivered::Held));
    let done = site
        .done_with_messages("send_message", to("j1"), Some(port))
        .await;
    assert!(!done.error, "没人看着不算出错");
    assert_eq!(
        text(&done),
        "Message saved for j1. Nobody is watching that one-shot session, so it reads this only when someone continues it.\n"
    );
}

#[tokio::test]
async fn every_refusal_says_why() {
    let site = Site::new();
    let cases = [
        (NotSent::NoParent, "parent", "This session has no parent.\n"),
        (
            NotSent::NotYours,
            "j7",
            "\"j7\" is not a subagent you started. Message only your own subagents or your parent.\n",
        ),
        (
            NotSent::Stopped,
            "j1",
            "Subagent j1 was stopped and takes no more messages.\n",
        ),
        (
            NotSent::Undelivered,
            "j1",
            "The message could not be delivered.\n",
        ),
        (
            NotSent::TooMany,
            "j1",
            "Too many messages to j1 just now. Put the rest into one message and send it later.\n",
        ),
        (
            NotSent::InboxFull,
            "j1",
            "j1 has too many unread messages. Send again after it has read them.\n",
        ),
    ];
    for (why, recipient, said) in cases {
        let port = Port::new(Err(why));
        let done = site
            .done_with_messages("send_message", to(recipient), Some(port.clone()))
            .await;
        assert!(done.error, "{why:?}");
        assert_eq!(text(&done), said, "{why:?}");
        assert_eq!(port.asked().len(), 1, "{why:?}：端口问过一次");
    }
}

#[tokio::test]
async fn a_duplicate_message_is_not_an_error() {
    let site = Site::new();
    let port = Port::new(Err(NotSent::Duplicate));
    let done = site
        .done_with_messages("send_message", to("j1"), Some(port))
        .await;
    assert!(!done.error, "那句话已经在那边了，不是出错");
    assert_eq!(text(&done), "j1 already has this exact message.\n");
}

#[tokio::test]
async fn without_a_port_it_is_not_delivered() {
    let site = Site::new();
    let done = site
        .done_with_messages("send_message", to("j1"), None)
        .await;
    assert!(done.error);
    assert_eq!(text(&done), "The message could not be delivered.\n");
}

#[tokio::test]
async fn without_both_arguments_nothing_is_asked() {
    let site = Site::new();
    let port = Port::new(Ok(Delivered::Sent));
    for args in [
        json!({"to": "j1"}),
        json!({"message": "hi"}),
        json!({"to": 1, "message": "hi"}),
    ] {
        let done = site
            .done_with_messages("send_message", args, Some(port.clone()))
            .await;
        assert!(done.error);
        assert!(text(&done).starts_with("The arguments are not right: "));
    }
    assert!(port.asked().is_empty());
}

#[tokio::test]
async fn a_message_over_the_character_limit_is_rejected_before_sending() {
    let site = Site::new();
    let port = Port::new(Ok(Delivered::Sent));
    let long = "a".repeat(100_001);
    let done = site
        .done_with_messages(
            "send_message",
            json!({"to": "j1", "message": long}),
            Some(port.clone()),
        )
        .await;
    assert!(done.error);
    assert_eq!(
        text(&done),
        "The message has 100001 characters, over the limit of 100000. Send a shorter one.\n"
    );
    assert!(port.asked().is_empty(), "超长的发出去之前拒");
    // 刚好到上限的：照发不误。
    let exact = "a".repeat(100_000);
    let done = site
        .done_with_messages(
            "send_message",
            json!({"to": "j1", "message": exact}),
            Some(port.clone()),
        )
        .await;
    assert!(!done.error, "刚好到上限不算超");
}

#[tokio::test]
async fn every_outcome_says_something_people_can_read() {
    let site = Site::new();
    let mut checked = Vec::new();
    let sent = site
        .done_with_messages(
            "send_message",
            to("j1"),
            Some(Port::new(Ok(Delivered::Sent))),
        )
        .await;
    check(
        &mut checked,
        human(sent),
        said("send_message/sent").with("to", "j1"),
    );
    let held = site
        .done_with_messages(
            "send_message",
            to("j1"),
            Some(Port::new(Ok(Delivered::Held))),
        )
        .await;
    check(
        &mut checked,
        human(held),
        said("send_message/held").with("to", "j1"),
    );
    for (why, key) in [
        (NotSent::NoParent, "no-parent"),
        (NotSent::NotYours, "not-yours"),
        (NotSent::Stopped, "stopped"),
        (NotSent::Undelivered, "not-sent"),
        (NotSent::TooMany, "too-many"),
        (NotSent::Duplicate, "duplicate"),
        (NotSent::InboxFull, "inbox-full"),
    ] {
        let done = site
            .done_with_messages("send_message", to("j1"), Some(Port::new(Err(why))))
            .await;
        check(
            &mut checked,
            human(done),
            said(&format!("send_message/{key}")).with("to", "j1"),
        );
    }
    let not_here = site
        .done_with_messages(
            "send_message",
            to("sibling"),
            Some(Port::new(Ok(Delivered::Sent))),
        )
        .await;
    check(&mut checked, human(not_here), said("send_message/not-here"));
    let long = "a".repeat(100_001);
    let too_long = site
        .done_with_messages(
            "send_message",
            json!({"to": "j1", "message": long}),
            Some(Port::new(Ok(Delivered::Sent))),
        )
        .await;
    check(
        &mut checked,
        human(too_long),
        said("send_message/too-long")
            .with("chars", "100001")
            .with("limit", "100000"),
    );
    readable(&checked, &["send_message", "message_agent"]);
}
