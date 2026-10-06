//! OpenAI Responses 接口的编码（`docs/blueprint/drivers/openai-responses.md`「怎么走：编码」，施工 8-13）：每一种写法一个样本，
//! 逐字节比对。思考在 `openai_responses_reasoning.rs`，图片、文件在 `openai_responses_media.rs`。

mod support;

use std::collections::BTreeMap;

use miyu_drivers::openai_responses::{PATH, encode};
use miyu_drivers::{Driver, Encoded, Inputs, OpenAiResponses};
use miyu_kernel::block::Block;
use miyu_kernel::request::{Message, Request};
use serde_json::{Value, json};
use support::{family_call, gpt, id, read_tool, responses_sample, text, texts, tool_call};

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
        &gpt(Inputs::default(), max_output),
        &texts(),
        &BTreeMap::new(),
    )
    .expect("不要 blob 的请求编码得了")
}

fn parsed(request: &Request, max_output: Option<u32>) -> Value {
    serde_json::from_slice(&encoded(request, max_output).body).expect("请求字节是 JSON")
}

fn chat() -> Request {
    request(
        false,
        vec![
            user(vec![text("你好")]),
            assistant(vec![text("你好！")]),
            user(vec![text("今天天气"), text("怎么样？")]),
        ],
    )
}

#[test]
fn plain_text() {
    responses_sample("plain-text", &encoded(&chat(), None).body);
    let value = parsed(&chat(), None);
    assert_eq!(value["instructions"], "You are a helpful assistant.");
    assert_eq!(
        value["input"],
        json!([
            {"role": "user", "content": "你好"},
            {"role": "assistant", "content": "你好！"},
            {"role": "user", "content": "今天天气\n怎么样？"}
        ])
    );
    assert_eq!(value["store"], false);
    assert!(value.get("tools").is_none());
    assert!(value.get("max_output_tokens").is_none());
}

#[test]
fn the_top_level_fields_come_in_a_fixed_order_and_nothing_else() {
    let request = request(true, chat().messages);
    let body = String::from_utf8(encoded(&request, Some(100)).body).expect("UTF-8");
    let keys = [
        "\"model\":",
        "\"instructions\":",
        "\"input\":",
        "\"tools\":",
        "\"store\":false",
        "\"stream\":true",
        "\"max_output_tokens\":100",
    ];
    let at: Vec<usize> = keys
        .iter()
        .map(|key| body.find(key).unwrap_or_else(|| panic!("没有 {key}")))
        .collect();
    assert!(at.windows(2).all(|pair| pair[0] < pair[1]), "{body}");
    assert!(body.ends_with(",\"max_output_tokens\":100}"), "{body}");
    let mut top: Vec<String> = parsed(&request, Some(100))
        .as_object()
        .expect("是对象")
        .keys()
        .cloned()
        .collect();
    top.sort_unstable();
    assert_eq!(
        top,
        [
            "input",
            "instructions",
            "max_output_tokens",
            "model",
            "store",
            "stream",
            "tools"
        ]
    );
    responses_sample("max-output", &encoded(&chat(), Some(4096)).body);
}

#[test]
fn an_empty_system_prompt_is_left_out() {
    let mut empty = chat();
    empty.system.clear();
    assert!(parsed(&empty, None).get("instructions").is_none());
}

#[test]
fn the_tools_are_not_strict_and_sent_when_any_or_called() {
    let face = parsed(&request(true, chat().messages), None);
    assert_eq!(
        face["tools"],
        json!([{
            "type": "function",
            "name": "read",
            "description": "Read a text file.",
            "parameters": {"type": "object", "properties": {"path": {"type": "string"}}, "required": ["path"]},
            "strict": false
        }])
    );
    let called = request(
        false,
        vec![
            user(vec![text("看看")]),
            assistant(vec![tool_call("call_1_1", "read", "{}", None)]),
            tool("call_1_1", false, vec![text("ok")]),
        ],
    );
    assert_eq!(parsed(&called, None)["tools"], json!([]));
    responses_sample("empty-tools", &encoded(&called, None).body);
}

#[test]
fn tool_calls_use_their_own_ids_the_kernels_or_nothing_foreign() {
    let request = request(
        true,
        vec![
            user(vec![text("读两个文件")]),
            assistant(vec![
                text("好的，"),
                text("我读一下。"),
                family_call(
                    "openai-responses",
                    "call_1_1",
                    "read",
                    r#"{"path":"a.txt"}"#,
                    None,
                ),
                // 别家的私有数据，写法和自己家一样也不认。
                Block::ToolCall(miyu_kernel::block::ToolCall {
                    call_id: id("call_1_2"),
                    name: "read".to_string(),
                    args: "not json".to_string(),
                    private: Some(support::private(
                        "openai-chat",
                        r#"{"call_id":"call_foreign"}"#,
                    )),
                }),
                text("读完了。"),
            ]),
            tool("call_1_1", false, vec![text("A")]),
            tool("call_1_2", true, vec![]),
        ],
    );
    let own = {
        // 自己家的编号写在 `call_id` 里。
        let mut request = request.clone();
        if let Message::Assistant { blocks } = &mut request.messages[1] {
            blocks[2] = Block::ToolCall(miyu_kernel::block::ToolCall {
                call_id: id("call_1_1"),
                name: "read".to_string(),
                args: r#"{"path":"a.txt"}"#.to_string(),
                private: Some(support::private(
                    "openai-responses",
                    r#"{"call_id":"call_abc"}"#,
                )),
            });
        }
        request
    };
    responses_sample("tool-calls", &encoded(&own, None).body);
    let value = parsed(&own, None);
    assert_eq!(
        value["input"],
        json!([
            {"role": "user", "content": "读两个文件"},
            {"role": "assistant", "content": "好的，我读一下。"},
            {"type": "function_call", "call_id": "call_abc", "name": "read", "arguments": "{\"path\":\"a.txt\"}"},
            {"type": "function_call", "call_id": "call_1_2", "name": "read", "arguments": "{}"},
            {"role": "assistant", "content": "读完了。"},
            {"type": "function_call_output", "call_id": "call_abc", "output": "A"},
            {"type": "function_call_output", "call_id": "call_1_2", "output": texts().no_output()}
        ])
    );
    // 别家的私有数据不认，`family_call` 那一次没有编号：用内核的。
    assert_eq!(parsed(&request, None)["input"][2]["call_id"], "call_1_1");
}

#[test]
fn every_input_item_has_its_range() {
    let encoded = encoded(&request(true, chat().messages), None);
    assert_eq!(encoded.messages.len(), 3);
    for range in &encoded.messages {
        let item: Value =
            serde_json::from_slice(&encoded.body[range.clone()]).expect("一项是一个 JSON");
        assert!(item["role"].is_string());
    }
    assert!(encoded.body[..encoded.messages[0].start].ends_with(b"\"input\":["));
}

#[test]
fn the_continuation_mark_changes_nothing() {
    let mut continuing = chat();
    continuing.continuation = true;
    let once = encoded(&continuing, None);
    assert_eq!(once.body, encoded(&chat(), None).body);
    assert_eq!(once.path, PATH);
    assert_eq!(PATH, "/responses");
}

#[test]
fn the_driver_says_its_family_headers_and_model_list() {
    let driver = OpenAiResponses::new(texts());
    assert_eq!(driver.family(), "openai-responses");
    assert_eq!(
        driver.auth("sk-test"),
        vec![("Authorization".to_string(), "Bearer sk-test".to_string())]
    );
    assert_eq!(driver.models_path(), "/models");
    let listed = driver
        .parse_models(br#"{"object":"list","data":[{"id":"gpt-5.4","object":"model"}]}"#)
        .expect("读得了");
    assert_eq!(listed[0].id, "gpt-5.4");
    let through = driver
        .encode(&chat(), &gpt(Inputs::default(), None), &BTreeMap::new())
        .expect("编码得了");
    assert_eq!(through, encoded(&chat(), None));
}
