//! 接着写被打断的回复（`docs/designs/05-内核接口.md` 第七节「接着写被打断的回复」，施工 3-5 再补）：
//! 带着记号、供应商又会接着写的，最后那句提示不发，半截那条加上字段，发到另一条路径；别的一字不变。

use std::collections::BTreeMap;

use crate::support::{call, deepseek, sample, text, texts, thought};
use miyu_drivers::Inputs;
use miyu_drivers::openai_chat::{Compat, Continuation, ContinuationField, Encoded, PATH, encode};
use miyu_kernel::request::{Message, Request};
use serde_json::Value;

/// 被打断的那一句，照资源里的写。
const NOTICE: &str = "<reply-cut>The reply above was cut off before it was finished. The user has already seen it. Continue from exactly where it stopped, without repeating it.</reply-cut>\n";

/// 数到六，说到「一二三」断了，带着半截和那一句再来。
fn cut(continuation: bool) -> Request {
    Request {
        tools: vec![],
        system: "You are a helpful assistant.".to_string(),
        messages: vec![
            Message::User {
                blocks: vec![text("数到六")],
            },
            Message::Assistant {
                blocks: vec![thought("从一开始数"), text("一二三")],
            },
            Message::User {
                blocks: vec![text(NOTICE)],
            },
        ],
        stable: 0,
        continuation,
        described: Default::default(),
        output_cap: None,
    }
}

fn encoded(request: &Request, compat: &Compat) -> Encoded {
    encode(
        request,
        &call(Inputs::default(), None),
        compat,
        &texts(),
        &BTreeMap::new(),
    )
    .expect("不要 blob 的请求编码得了")
}

fn messages(encoded: &Encoded) -> Vec<Value> {
    let body: Value = serde_json::from_slice(&encoded.body).expect("是 JSON");
    body["messages"].as_array().expect("有 messages").clone()
}

#[test]
fn deepseek_continues_the_half_with_its_thinking() {
    let encoded = encoded(&cut(true), &deepseek());
    sample("continuation-deepseek", &encoded.body);
    assert_eq!(encoded.path, "/beta/chat/completions");
    let messages = messages(&encoded);
    // system、人说的、半截：那句提示不发。
    assert_eq!(messages.len(), 3);
    let half = &messages[2];
    assert_eq!(half["role"], "assistant");
    assert_eq!(half["content"], "一二三");
    assert_eq!(half["reasoning_content"], "从一开始数");
    assert_eq!(half["prefix"], true);
    assert_eq!(encoded.messages.len(), 3, "每条线上的消息都找得到位置");
}

#[test]
fn without_the_switch_or_the_marker_nothing_changes() {
    // 不会接着写的：带着记号也照原样发，那句提示在最后。
    let off = Compat {
        continuation: Continuation::None,
        ..deepseek()
    };
    let plain = encoded(&cut(false), &deepseek());
    assert_eq!(encoded(&cut(true), &off).body, plain.body);
    assert_eq!(encoded(&cut(true), &off).path, PATH);
    // 没有记号的：会接着写的也照原样发。
    assert_eq!(plain.path, PATH);
    let messages = messages(&plain);
    assert_eq!(messages.len(), 4);
    assert_eq!(messages[3]["content"], NOTICE);
    assert!(messages[2].get("prefix").is_none());
}

#[test]
fn the_partial_style_writes_its_own_field() {
    // Kimi、通义的写法：字段是 `partial`，路径照开关里写的。出厂没有打开，这里只查写法。
    let partial = Compat {
        continuation: Continuation::Prefix {
            field: ContinuationField::Partial,
            path: PATH.to_string(),
        },
        ..deepseek()
    };
    let encoded = encoded(&cut(true), &partial);
    let half = &messages(&encoded)[2];
    assert_eq!(half["partial"], true);
    assert!(half.get("prefix").is_none());
    assert_eq!(encoded.path, PATH);
}
