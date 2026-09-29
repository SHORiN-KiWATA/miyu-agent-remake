//! `message_agent`（`docs/blueprint/tools/message_agent.md`，施工 7-7）：只声明 `to`、`message`；`parent` 发给父会话，任务
//! 编号发给自己派的子代理，经端口原样送出去，送到了说一句，给子代理的报 `job.messaged`；别的都拒，每一种说清为什么：没有父、不是她派的（写法都不对的
//! 端口不问）、被停掉了、送不到、没有端口；参数不对的照共用的那一句。给人看的说法两种语言都换得出字。

mod support;

use std::sync::{Arc, Mutex, PoisonError};

use serde_json::json;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::JobMessaged;
use miyu_kernel::id::JobId;
use miyu_kernel::tool::Access;
use miyu_tool::{Done, Effect, MessagePort, NotSent, Recipient, Sending};

use support::{Site, check, human, readable, said, tool};

/// 假的端口：记下交给它的每一次发给谁、说了什么，照 `answer` 回。
struct Port {
    answer: Result<(), NotSent>,
    asked: Mutex<Vec<(Recipient, String)>>,
}

impl Port {
    fn new(answer: Result<(), NotSent>) -> Arc<Port> {
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
fn it_declares_only_the_recipient_and_the_message() {
    let tool = tool("message_agent");
    let spec = tool.spec();
    assert_eq!(spec.name, "message_agent");
    // 留言什么都不改：只读开着也发得出去，给几个子代理留言连着的一起发。
    assert_eq!(spec.access, Access::Read);
    let parameters: serde_json::Value = serde_json::from_str(spec.parameters.get()).unwrap();
    let names: Vec<&String> = parameters["properties"]
        .as_object()
        .unwrap()
        .keys()
        .collect();
    assert_eq!(names, ["message", "to"]);
    assert_eq!(parameters["required"], json!(["to", "message"]));
    assert!(
        spec.description
            .contains("Send only what they need to know now"),
        "只发对方现在就得知道的那一句留着：{}",
        spec.description
    );
}

#[tokio::test]
async fn it_sends_to_a_subagent_or_the_parent_as_it_is() {
    let site = Site::new();
    let port = Port::new(Ok(()));
    let done = site
        .done_with_messages("message_agent", to("j2"), Some(port.clone()))
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
        .done_with_messages("message_agent", to("parent"), Some(port.clone()))
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
    ];
    for (why, recipient, said) in cases {
        let port = Port::new(Err(why));
        let done = site
            .done_with_messages("message_agent", to(recipient), Some(port.clone()))
            .await;
        assert!(done.error, "{why:?}");
        assert_eq!(text(&done), said, "{why:?}");
        assert_eq!(port.asked().len(), 1, "{why:?}：端口问过一次");
    }
}

#[tokio::test]
async fn a_recipient_that_is_neither_parent_nor_a_job_id_is_not_asked() {
    let site = Site::new();
    let port = Port::new(Ok(()));
    for recipient in [
        "sibling",
        "J1",
        "j0",
        " j1",
        "Parent",
        "",
        "01a0d78c-ca52-7d19",
    ] {
        let done = site
            .done_with_messages("message_agent", to(recipient), Some(port.clone()))
            .await;
        assert!(done.error);
        assert!(
            text(&done).ends_with(
                "is not a subagent you started. Message only your own subagents or your parent.\n"
            ),
            "{recipient:?}：{}",
            text(&done)
        );
    }
    assert!(port.asked().is_empty(), "不会是她派的：端口一次都不问");
}

#[tokio::test]
async fn without_a_port_it_is_not_delivered() {
    let site = Site::new();
    let done = site
        .done_with_messages("message_agent", to("j1"), None)
        .await;
    assert!(done.error);
    assert_eq!(text(&done), "The message could not be delivered.\n");
}

#[tokio::test]
async fn without_both_arguments_nothing_is_asked() {
    let site = Site::new();
    let port = Port::new(Ok(()));
    for args in [
        json!({"to": "j1"}),
        json!({"message": "hi"}),
        json!({"to": 1, "message": "hi"}),
    ] {
        let done = site
            .done_with_messages("message_agent", args, Some(port.clone()))
            .await;
        assert!(done.error);
        assert!(text(&done).starts_with("The arguments are not right: "));
    }
    assert!(port.asked().is_empty());
}

#[tokio::test]
async fn every_outcome_says_something_people_can_read() {
    let site = Site::new();
    let mut checked = Vec::new();
    let sent = site
        .done_with_messages("message_agent", to("j1"), Some(Port::new(Ok(()))))
        .await;
    check(
        &mut checked,
        human(sent),
        said("message_agent/sent").with("to", "j1"),
    );
    for (why, key) in [
        (NotSent::NoParent, "no-parent"),
        (NotSent::NotYours, "not-yours"),
        (NotSent::Stopped, "stopped"),
        (NotSent::Undelivered, "not-sent"),
    ] {
        let done = site
            .done_with_messages("message_agent", to("j1"), Some(Port::new(Err(why))))
            .await;
        check(
            &mut checked,
            human(done),
            said(&format!("message_agent/{key}")).with("to", "j1"),
        );
    }
    readable(&checked, &["message_agent"]);
}
