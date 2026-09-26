//! 统一请求的测试：同样的请求字节一样；参数格式原样照抄；指纹说得出第一处不同在哪，
//! 只在后面接着加的没有不同，少了的从少了的那一条算。

use super::*;
use crate::block::{Text, ToolCall};

fn text(text: &str) -> Block {
    Block::Text(Text {
        text: text.to_string(),
    })
}

fn call() -> CallId {
    CallId::parse("call_2_1").unwrap()
}

/// 一份三条消息的请求：人说一句，模型调一个工具，工具的结果。
fn request() -> Request {
    Request {
        tools: vec![ToolSpec {
            name: "read".to_string(),
            description: "Read a file.".to_string(),
            parameters: serde_json::from_str(r#"{"type":"object"}"#).unwrap(),
        }],
        system: "You are a helpful software engineer.".to_string(),
        messages: vec![
            Message::User {
                blocks: vec![text("看看 src 目录")],
            },
            Message::Assistant {
                blocks: vec![Block::ToolCall(ToolCall {
                    call_id: call(),
                    name: "read".to_string(),
                    args: r#"{"path":"src"}"#.to_string(),
                    private: None,
                })],
            },
            Message::Tool {
                call_id: call(),
                error: false,
                blocks: vec![text("lib.rs")],
            },
        ],
        stable: 0,
    }
}

#[test]
fn the_same_request_has_the_same_bytes_and_hash() {
    assert_eq!(request().canonical_bytes(), request().canonical_bytes());
    assert_eq!(
        request().hash(),
        ContentHash::of(&request().canonical_bytes())
    );
}

/// 参数格式原样照抄，连空格都不动：重新写一遍会改变字节。
#[test]
fn tool_parameters_are_kept_byte_for_byte() {
    let mut request = request();
    request.tools[0].parameters = serde_json::from_str(r#"{ "type" : "object" }"#).unwrap();
    let bytes = String::from_utf8(request.canonical_bytes()).unwrap();
    assert!(
        bytes.contains(r#""parameters":{ "type" : "object" }"#),
        "{bytes}"
    );
}

/// 每条消息写出来都以角色开头，角色的写法和 `Role::as_str` 一样。
#[test]
fn every_message_is_written_role_first() {
    for message in request().messages {
        let bytes = String::from_utf8(json(&message)).unwrap();
        let head = format!(r#"{{"role":"{}","#, message.role().as_str());
        assert!(bytes.starts_with(&head), "{bytes}");
    }
}

#[test]
fn a_request_that_only_grows_has_no_difference() {
    let before = request().fingerprint();
    assert_eq!(request().fingerprint().first_difference(&before), None);
    let mut longer = request();
    longer.messages.push(Message::User {
        blocks: vec![text("只看 .rs 文件")],
    });
    assert_eq!(longer.fingerprint().first_difference(&before), None);
}

#[test]
fn tools_and_system_are_compared_first() {
    let before = request().fingerprint();
    let mut changed = request();
    changed.tools[0].description = "Read a file or a directory.".to_string();
    changed.messages.clear();
    assert_eq!(
        changed.fingerprint().first_difference(&before),
        Some(Difference::Tools)
    );
    let mut changed = request();
    changed.system.push_str(" Answer briefly.");
    assert_eq!(
        changed.fingerprint().first_difference(&before),
        Some(Difference::System)
    );
}

#[test]
fn a_changed_message_is_reported_with_its_index_and_role() {
    let before = request().fingerprint();
    let mut changed = request();
    changed.messages[2] = Message::Tool {
        call_id: call(),
        error: true,
        blocks: vec![text("lib.rs")],
    };
    assert_eq!(
        changed.fingerprint().first_difference(&before),
        Some(Difference::Message {
            index: 2,
            role: Role::Tool
        })
    );
}

#[test]
fn a_missing_message_counts_from_where_it_went_missing() {
    let before = request().fingerprint();
    let mut shorter = request();
    shorter.messages.truncate(1);
    assert_eq!(
        shorter.fingerprint().first_difference(&before),
        Some(Difference::Message {
            index: 1,
            role: Role::Assistant
        })
    );
}
