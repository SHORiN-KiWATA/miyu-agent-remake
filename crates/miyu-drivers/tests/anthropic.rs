//! Anthropic 消息接口的编码（`docs/blueprint/drivers/anthropic.md`「怎么走：编码」，施工 8-12）：每一种写法一个样本，
//! 逐字节比对。打点在 `anthropic_marks.rs`，思考在 `anthropic_thinking.rs`，图片、文件在 `anthropic_media.rs`。

use std::collections::BTreeMap;

use crate::support::{
    anthropic_sample, claude, family_call, id, read_tool, text, texts, tool_call,
};
use miyu_drivers::anthropic::{FALLBACK_MAX_TOKENS, PATH, encode};
use miyu_drivers::{Anthropic, Driver, Encoded, Inputs};
use miyu_kernel::block::Block;
use miyu_kernel::request::{Message, Request};
use serde_json::{Value, json};

fn user(blocks: Vec<Block>) -> Message {
    Message::User { blocks }
}

fn assistant(blocks: Vec<Block>) -> Message {
    Message::Assistant { blocks }
}

fn tool(call_id: &str, error: bool, blocks: Vec<Block>) -> Message {
    Message::Tool {
        call_id: id(call_id),
        error,
        blocks,
    }
}

fn request(tools: bool, messages: Vec<Message>) -> Request {
    Request {
        tools: if tools { vec![read_tool()] } else { vec![] },
        system: "You are a helpful assistant.".to_string(),
        messages,
        stable: 0,
        continuation: false,
        described: Default::default(),
    }
}

fn encoded(request: &Request, max_output: Option<u32>) -> Encoded {
    encode(
        request,
        &claude(Inputs::default(), max_output),
        &texts(),
        &BTreeMap::new(),
    )
    .expect("不要 blob 的请求编码得了")
}

/// 去掉打点的请求字节读成 JSON：查结构用，打点由 `anthropic_marks.rs` 查。
fn parsed(request: &Request, max_output: Option<u32>) -> Value {
    let body = encoded(request, max_output).body;
    let text = String::from_utf8(body).expect("请求字节是 UTF-8");
    serde_json::from_str(&text.replace(r#","cache_control":{"type":"ephemeral"}"#, ""))
        .expect("请求字节是 JSON")
}

fn chat() -> Request {
    request(
        false,
        vec![
            user(vec![text("你好")]),
            assistant(vec![text("你好！")]),
            user(vec![text("今天天气怎么样？")]),
        ],
    )
}

#[test]
fn plain_text() {
    // 没有工具：不发 tools；max_tokens 照调用。
    anthropic_sample("plain-text", &encoded(&chat(), Some(4096)).body);
}

#[test]
fn without_an_output_limit_it_writes_the_fallback() {
    assert_eq!(FALLBACK_MAX_TOKENS, 8192);
    anthropic_sample("max-tokens-fallback", &encoded(&chat(), None).body);
    assert_eq!(parsed(&chat(), None)["max_tokens"], 8192);
    assert_eq!(parsed(&chat(), Some(64000))["max_tokens"], 64000);
}

#[test]
fn the_top_level_fields_come_in_a_fixed_order_and_nothing_else() {
    let request = request(true, chat().messages);
    let body = String::from_utf8(encoded(&request, Some(100)).body).expect("UTF-8");
    let keys = [
        "\"model\":",
        "\"max_tokens\":",
        "\"system\":",
        "\"tools\":",
        "\"messages\":",
    ];
    let at: Vec<usize> = keys
        .iter()
        .map(|key| body.find(key).unwrap_or_else(|| panic!("没有 {key}")))
        .collect();
    assert!(at.windows(2).all(|pair| pair[0] < pair[1]), "{body}");
    assert!(body.ends_with(r#"],"stream":true}"#), "{body}");
    let top: Vec<String> = parsed(&request, Some(100))
        .as_object()
        .expect("是对象")
        .keys()
        .cloned()
        .collect();
    let mut want = vec![
        "max_tokens",
        "messages",
        "model",
        "stream",
        "system",
        "tools",
    ];
    want.sort_unstable();
    let mut top = top;
    top.sort_unstable();
    assert_eq!(top, want);
}

#[test]
fn the_system_prompt_is_one_text_block_and_an_empty_one_is_left_out() {
    let value = parsed(&chat(), None);
    assert_eq!(
        value["system"],
        json!([{"type": "text", "text": "You are a helpful assistant."}])
    );
    let mut empty = chat();
    empty.system.clear();
    assert!(parsed(&empty, None).get("system").is_none());
}

#[test]
fn the_tools_are_sent_when_there_are_any_or_the_history_calls_them() {
    let face = parsed(&request(true, chat().messages), None);
    assert_eq!(
        face["tools"],
        json!([{
            "name": "read",
            "description": "Read a text file.",
            "input_schema": {"type": "object", "properties": {"path": {"type": "string"}}, "required": ["path"]}
        }])
    );
    // 参数格式原样照抄：字节里就是资源里的那一段。
    let body =
        String::from_utf8(encoded(&request(true, chat().messages), None).body).expect("UTF-8");
    assert!(body.contains(
        r#""input_schema":{"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}"#
    ));
    assert!(parsed(&chat(), None).get("tools").is_none());
    let called = request(
        false,
        vec![
            user(vec![text("看看")]),
            assistant(vec![tool_call("call_1_1", "read", "{}", None)]),
            tool("call_1_1", false, vec![text("ok")]),
        ],
    );
    assert_eq!(parsed(&called, None)["tools"], json!([]));
    anthropic_sample("empty-tools", &encoded(&called, None).body);
}

#[test]
fn tool_calls_use_their_own_ids_the_kernels_or_nothing_foreign() {
    let request = request(
        true,
        vec![
            user(vec![text("读两个文件")]),
            assistant(vec![
                text("好的。"),
                family_call(
                    "anthropic",
                    "call_1_1",
                    "read",
                    r#"{"path":"a.txt"}"#,
                    Some("toolu_01A"),
                ),
                tool_call(
                    "call_1_2",
                    "read",
                    r#"{"path":"b.txt"}"#,
                    Some("call.foreign:9"),
                ),
                family_call("anthropic", "call_1_3", "read", "not json", None),
                family_call("anthropic", "call_1_4", "read", "[1,2]", None),
            ]),
            tool("call_1_1", false, vec![text("A")]),
            tool("call_1_2", true, vec![text("no such file")]),
            tool("call_1_3", false, vec![]),
            tool("call_1_4", false, vec![text("")]),
        ],
    );
    anthropic_sample("tool-calls", &encoded(&request, None).body);
    let value = parsed(&request, None);
    let calls = &value["messages"][1]["content"];
    assert_eq!(calls[0], json!({"type": "text", "text": "好的。"}));
    assert_eq!(
        calls[1],
        json!({"type": "tool_use", "id": "toolu_01A", "name": "read", "input": {"path": "a.txt"}})
    );
    assert_eq!(calls[2]["id"], "call_1_2");
    assert_eq!(calls[3]["input"], json!({}));
    assert_eq!(calls[4]["input"], json!({}));
    let results = &value["messages"][2]["content"];
    assert_eq!(value["messages"].as_array().map(Vec::len), Some(3));
    assert_eq!(value["messages"][2]["role"], "user");
    assert_eq!(
        results[0],
        json!({"type": "tool_result", "tool_use_id": "toolu_01A", "content": [{"type": "text", "text": "A"}]})
    );
    assert_eq!(results[1]["tool_use_id"], "call_1_2");
    assert_eq!(results[1]["is_error"], true);
    let no_output = texts().no_output();
    assert_eq!(
        results[2]["content"],
        json!([{"type": "text", "text": no_output}])
    );
    assert_eq!(
        results[3]["content"],
        json!([{"type": "text", "text": no_output}])
    );
}

#[test]
fn the_argument_text_of_a_call_is_kept_byte_for_byte() {
    let args = r#"{ "path" : "a.txt",  "n":1.50 }"#;
    let request = request(
        true,
        vec![
            user(vec![text("读")]),
            assistant(vec![tool_call("call_1_1", "read", args, None)]),
            tool("call_1_1", false, vec![text("A")]),
        ],
    );
    let body = String::from_utf8(encoded(&request, None).body).expect("UTF-8");
    assert!(body.contains(&format!(r#""input":{args}"#)), "{body}");
    // 前后的空白不算原文：嵌进请求里的是那个对象本身。
    let padded = request_with_args(" {\"a\":1}\n");
    let body = String::from_utf8(encoded(&padded, None).body).expect("UTF-8");
    assert!(body.contains(r#""input":{"a":1}}"#), "{body}");
}

fn request_with_args(args: &str) -> Request {
    request(
        true,
        vec![
            user(vec![text("读")]),
            assistant(vec![tool_call("call_1_1", "read", args, None)]),
            tool("call_1_1", false, vec![text("A")]),
        ],
    )
}

#[test]
fn neighbours_with_the_same_role_are_merged_and_empty_messages_are_left_out() {
    let request = request(
        true,
        vec![
            user(vec![text("读一下")]),
            assistant(vec![
                tool_call("call_1_1", "read", "{}", None),
                tool_call("call_1_2", "read", "{}", None),
            ]),
            tool("call_1_1", false, vec![text("A")]),
            tool("call_1_2", false, vec![text("B")]),
            user(vec![text("还有，")]),
            assistant(vec![text("")]),
            user(vec![text(""), text("快一点")]),
        ],
    );
    anthropic_sample("merged", &encoded(&request, None).body);
    let value = parsed(&request, None);
    let messages = value["messages"].as_array().expect("是数组");
    assert_eq!(messages.len(), 3);
    let roles: Vec<&str> = messages
        .iter()
        .map(|message| message["role"].as_str().expect("有角色"))
        .collect();
    assert_eq!(roles, ["user", "assistant", "user"]);
    let last: Vec<&str> = messages[2]["content"]
        .as_array()
        .expect("是数组")
        .iter()
        .map(|block| block["type"].as_str().expect("有种类"))
        .collect();
    assert_eq!(last, ["tool_result", "tool_result", "text", "text"]);
    assert_eq!(messages[2]["content"][3]["text"], "快一点");
}

#[test]
fn user_text_blocks_are_sent_one_by_one_as_they_are() {
    let request = request(
        false,
        vec![user(vec![text("<fact>\nnow\n</fact>\n"), text("你好")])],
    );
    assert_eq!(
        parsed(&request, None)["messages"][0]["content"],
        json!([
            {"type": "text", "text": "<fact>\nnow\n</fact>\n"},
            {"type": "text", "text": "你好"}
        ])
    );
}

#[test]
fn the_continuation_mark_changes_nothing() {
    let mut continuing = chat();
    continuing.continuation = true;
    let encoded_once = encoded(&continuing, None);
    assert_eq!(encoded_once.body, encoded(&chat(), None).body);
    assert_eq!(encoded_once.path, PATH);
    assert_eq!(PATH, "/messages");
}

#[test]
fn every_wire_message_has_its_range() {
    let request = request(true, chat().messages);
    let encoded = encoded(&request, None);
    assert_eq!(encoded.messages.len(), 3);
    let body = &encoded.body;
    for (range, role) in encoded.messages.iter().zip(["user", "assistant", "user"]) {
        let message: Value = serde_json::from_slice(&body[range.clone()]).expect("一条是一个 JSON");
        assert_eq!(message["role"], role);
    }
    assert!(body[..encoded.messages[0].start].ends_with(b"\"messages\":["));
}

#[test]
fn the_driver_says_its_family_headers_and_model_list() {
    let driver = Anthropic::new(texts());
    assert_eq!(driver.family(), "anthropic");
    assert_eq!(
        driver.auth("sk-ant-test"),
        vec![
            ("x-api-key".to_string(), "sk-ant-test".to_string()),
            ("anthropic-version".to_string(), "2023-06-01".to_string()),
        ]
    );
    assert_eq!(driver.models_path(), "/models?limit=1000");
    let via_driver = driver
        .encode(&chat(), &claude(Inputs::default(), None), &BTreeMap::new())
        .expect("编码得了");
    assert_eq!(via_driver, encoded(&chat(), None));
}
