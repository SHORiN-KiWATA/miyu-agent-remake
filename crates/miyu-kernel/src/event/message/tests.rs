//! 消息事件的测试：图纸上的 `message.assistant` 读写一字不差、认得出种类；
//! `seen` 必填；被打断的多写一格，没被打断的不写；坏的报错说清是哪一种。

use crate::block::Block;
use crate::event::{Body, Event};
use crate::test_support::{event_line, read_body, rejected};

const ASSISTANT: &str = r#"{"blocks":[{"type":"text","text":"我先看一下目录。"},{"type":"tool_call","call_id":"call_44_1","name":"read","args":"{\"path\":\"src\"}"}],"seen":43}"#;

#[test]
fn assistant_message_from_the_drawing_round_trips() {
    match read_body("message.assistant", ASSISTANT) {
        Body::MessageAssistant(message) => {
            assert!(!message.interrupted);
            assert_eq!(message.seen.get(), 43);
            assert!(matches!(message.blocks[1], Block::ToolCall(_)));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn interrupted_is_written_only_when_true() {
    let cut = r#"{"blocks":[{"type":"text","text":"我先看"}],"seen":43,"interrupted":true}"#;
    match read_body("message.assistant", cut) {
        Body::MessageAssistant(message) => assert!(message.interrupted),
        other => panic!("{other:?}"),
    }
    // 写成 false 的也读得进来，当没被打断；写出去就不带这一格了。
    let line = event_line(
        "message.assistant",
        r#"{"blocks":[],"seen":43,"interrupted":false}"#,
    );
    assert_eq!(
        Event::from_line(&line).unwrap().to_line(),
        event_line("message.assistant", r#"{"blocks":[],"seen":43}"#)
    );
}

#[test]
fn broken_assistant_bodies_say_which_kind() {
    // 少了 blocks；少了 seen；seen 从 1 开始；interrupted 不是布尔。
    for body in [
        r#"{"seen":43,"interrupted":true}"#,
        r#"{"blocks":[]}"#,
        r#"{"blocks":[],"seen":0}"#,
        r#"{"blocks":[],"seen":43,"interrupted":"yes"}"#,
    ] {
        let line = event_line("message.assistant", body);
        rejected::<Event>(&line, "message.assistant 的 body 读不出来");
    }
}

#[test]
fn a_withdrawal_from_the_drawing_round_trips() {
    match read_body("message.withdrawn", r#"{"messages":[59]}"#) {
        Body::MessageWithdrawn(withdrawn) => {
            assert_eq!(
                withdrawn
                    .messages
                    .iter()
                    .map(|seq| seq.get())
                    .collect::<Vec<_>>(),
                [59]
            );
        }
        other => panic!("{other:?}"),
    }
    rejected::<Event>(
        &event_line("message.withdrawn", r#"{"messages":[0]}"#),
        "message.withdrawn 的 body 读不出来",
    );
}
