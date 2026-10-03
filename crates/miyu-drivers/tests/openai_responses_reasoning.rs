//! Responses 的思考（`docs/blueprint/drivers/openai-responses.md`「编码」第 4 条、「思考强度」，施工 8-13）：加密的思考回传、
//! 空摘要写 `[]`、别家的和没有加密内容的不写、和正文、调用的先后；思考强度两种写法接在最后，没写的、`on` 一个字节不加。

mod support;

use std::collections::BTreeMap;

use miyu_drivers::openai_responses::encode;
use miyu_drivers::{Call, EFFORT_OFF, EFFORT_ON, Inputs};
use miyu_kernel::block::Block;
use miyu_kernel::request::{Message, Request};
use serde_json::{Value, json};
use support::{family_call, gpt, private_thought, responses_sample, text, texts, thought};

fn request(reply: Vec<Block>) -> Request {
    Request {
        tools: vec![support::read_tool()],
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

fn encrypted(text: &str, id: &str) -> Block {
    private_thought(
        text,
        "openai-responses",
        &format!(r#"{{"id":"{id}","encrypted_content":"gAAA{id}"}}"#),
    )
}

#[test]
fn encrypted_thinking_goes_back_and_the_rest_is_dropped() {
    let request = request(vec![
        encrypted("先看目录", "rs_1"),
        text("我看一下。"),
        family_call("openai-responses", "call_2_1", "read", "{}", None),
        encrypted("", "rs_2"),
        thought("断在这里"),
        private_thought("别家想的", "anthropic", r#"{"signature":"x"}"#),
        // 别家的、数据长得一样的也不认。
        private_thought(
            "别家的加密",
            "openai-chat",
            r#"{"id":"rs_x","encrypted_content":"zzz"}"#,
        ),
        private_thought("只有编号", "openai-responses", r#"{"id":"rs_3"}"#),
        text("好的。"),
    ]);
    let bytes = body(&request, &gpt(Inputs::default(), None));
    responses_sample("reasoning", &bytes);
    let value: Value = serde_json::from_slice(&bytes).expect("JSON");
    assert_eq!(
        value["input"],
        json!([
            {"role": "user", "content": "想一想"},
            {"type": "reasoning", "id": "rs_1", "summary": [{"type": "summary_text", "text": "先看目录"}], "encrypted_content": "gAAArs_1"},
            {"role": "assistant", "content": "我看一下。"},
            {"type": "function_call", "call_id": "call_2_1", "name": "read", "arguments": "{}"},
            {"type": "reasoning", "id": "rs_2", "summary": [], "encrypted_content": "gAAArs_2"},
            {"role": "assistant", "content": "好的。"},
            {"role": "user", "content": "接着"}
        ])
    );
}

fn with_effort(effort: Option<&str>) -> String {
    let mut call = gpt(Inputs::default(), None);
    call.effort = effort.map(str::to_string);
    String::from_utf8(body(&request(vec![text("好的。")]), &call)).expect("UTF-8")
}

#[test]
fn no_effort_and_on_add_not_a_byte() {
    let plain = with_effort(None);
    assert!(
        plain.ends_with(r#","store":false,"stream":true}"#),
        "{plain}"
    );
    assert!(!plain.contains("reasoning"));
    assert_eq!(with_effort(Some(EFFORT_ON)), plain);
}

#[test]
fn a_level_asks_for_a_summary_and_the_encrypted_thinking() {
    let level = with_effort(Some("high"));
    responses_sample("effort-level", level.as_bytes());
    assert!(level.ends_with(
        r#","stream":true,"reasoning":{"effort":"high","summary":"auto"},"include":["reasoning.encrypted_content"]}"#
    ));
}

#[test]
fn off_is_effort_none() {
    let off = with_effort(Some(EFFORT_OFF));
    responses_sample("effort-off", off.as_bytes());
    assert!(off.ends_with(r#","stream":true,"reasoning":{"effort":"none"}}"#));
}
