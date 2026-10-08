//! Anthropic 的思考（`docs/blueprint/drivers/anthropic.md`「怎么走：编码」第 5 条、「思考强度」，施工 8-12）：思考块回传，
//! 带签名的、只有签名的、`redacted` 的原样，断了没签名的、别家的丢掉；思考强度四种写法，接在最后，没写的一个字节不加。

use std::collections::BTreeMap;

use crate::support::{anthropic_sample, claude, private_thought, text, texts, thought};
use miyu_drivers::anthropic::encode;
use miyu_drivers::{Call, EFFORT_OFF, EFFORT_ON, Inputs};
use miyu_kernel::block::Block;
use miyu_kernel::request::{Message, Request};
use serde_json::{Value, json};

fn request(reply: Vec<Block>) -> Request {
    Request {
        tools: vec![],
        system: "You are Miyu.".to_string(),
        messages: vec![
            Message::User {
                blocks: vec![text("想一想")],
            },
            Message::Assistant { blocks: reply },
            Message::User {
                blocks: vec![text("接着")],
            },
        ],
        stable: 0,
        continuation: false,
        described: Default::default(),
    }
}

fn body(request: &Request, call: &Call) -> Vec<u8> {
    encode(request, call, &texts(), &BTreeMap::new())
        .expect("不要 blob 的请求编码得了")
        .body
}

/// 请求字节去掉打点读成 JSON：打点由 `anthropic_marks.rs` 查。
fn parsed(request: &Request) -> Value {
    let text = String::from_utf8(body(request, &claude(Inputs::default(), None))).expect("UTF-8");
    serde_json::from_str(&text.replace(r#","cache_control":{"type":"ephemeral"}"#, ""))
        .expect("请求字节是 JSON")
}

fn reply_blocks(request: &Request) -> Value {
    parsed(request)["messages"][1]["content"].clone()
}

#[test]
fn thinking_goes_back_with_its_signature_and_the_rest_is_dropped() {
    let request = request(vec![
        private_thought("先看目录", "anthropic", r#"{"signature":"sig-A"}"#),
        private_thought("", "anthropic", r#"{"signature":"sig-B"}"#),
        private_thought("", "anthropic", r#"{"redacted":"ENCRYPTED"}"#),
        // 说到一半断了：只有字，没有签名。
        thought("断在这里"),
        // 别家的：私有数据不是这个驱动的。
        private_thought("别家想的", "openai-chat", r#"{"signature":"x"}"#),
        // 自己家的、数据里既没有签名也没有 redacted 的。
        private_thought("怪的", "anthropic", r#"{"id":"x"}"#),
        text("好的。"),
    ]);
    anthropic_sample(
        "thinking",
        &body(&request, &claude(Inputs::default(), None)),
    );
    assert_eq!(
        reply_blocks(&request),
        json!([
            {"type": "thinking", "thinking": "先看目录", "signature": "sig-A"},
            {"type": "thinking", "thinking": "", "signature": "sig-B"},
            {"type": "redacted_thinking", "data": "ENCRYPTED"},
            {"type": "text", "text": "好的。"}
        ])
    );
}

#[test]
fn a_reply_with_nothing_left_is_not_sent() {
    let request = request(vec![thought("断在这里")]);
    let value = parsed(&request);
    // 回复一块都没剩：不发，前后两条 user 合成一条。
    assert_eq!(value["messages"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        value["messages"][0]["content"],
        json!([{"type": "text", "text": "想一想"}, {"type": "text", "text": "接着"}])
    );
}

fn with_effort(effort: Option<&str>) -> Vec<u8> {
    let mut call = claude(Inputs::default(), None);
    call.effort = effort.map(str::to_string);
    body(&request(vec![text("好的。")]), &call)
}

#[test]
fn no_effort_adds_not_a_byte() {
    let plain = String::from_utf8(with_effort(None)).expect("UTF-8");
    assert!(plain.ends_with(r#"],"stream":true}"#));
    assert!(!plain.contains("thinking"));
    assert!(!plain.contains("output_config"));
}

#[test]
fn a_level_is_adaptive_summarized_thinking_and_the_effort_at_the_end() {
    let body = with_effort(Some("high"));
    anthropic_sample("effort-level", &body);
    let text = String::from_utf8(body).expect("UTF-8");
    assert!(text.ends_with(
        r#","stream":true,"thinking":{"type":"adaptive","display":"summarized"},"output_config":{"effort":"high"}}"#
    ));
}

#[test]
fn off_disables_thinking_and_on_turns_it_on() {
    let off = with_effort(Some(EFFORT_OFF));
    anthropic_sample("effort-off", &off);
    assert!(
        String::from_utf8(off)
            .expect("UTF-8")
            .ends_with(r#","stream":true,"thinking":{"type":"disabled"}}"#)
    );
    let on = with_effort(Some(EFFORT_ON));
    anthropic_sample("effort-on", &on);
    assert!(
        String::from_utf8(on)
            .expect("UTF-8")
            .ends_with(r#","stream":true,"thinking":{"type":"adaptive","display":"summarized"}}"#)
    );
}
