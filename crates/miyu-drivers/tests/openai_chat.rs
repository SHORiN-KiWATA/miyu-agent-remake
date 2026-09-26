//! OpenAI 兼容对话接口的编码（`docs/designs/05-内核接口.md` 第七节那张表）：每一种写法一个样本，
//! 逐字节比对。图片、文件和思考在 `openai_chat_media.rs`。

mod support;

use std::collections::BTreeMap;

use miyu_drivers::Inputs;
use miyu_drivers::openai_chat::{Compat, OutputLimit, encode};
use miyu_kernel::request::{Message, Request};
use support::{call, id, raw, read_tool, sample, text, texts, tool_call};

fn user(blocks: Vec<miyu_kernel::block::Block>) -> Message {
    Message::User { blocks }
}

fn assistant(blocks: Vec<miyu_kernel::block::Block>) -> Message {
    Message::Assistant { blocks }
}

fn tool(call_id: &str, blocks: Vec<miyu_kernel::block::Block>) -> Message {
    Message::Tool {
        call_id: id(call_id),
        error: false,
        blocks,
    }
}

fn request(tools: bool, messages: Vec<Message>) -> Request {
    Request {
        tools: if tools { vec![read_tool()] } else { vec![] },
        system: "You are a helpful assistant.".to_string(),
        messages,
        stable: 0,
    }
}

fn body(request: &Request, compat: &Compat, max_output: Option<u32>) -> Vec<u8> {
    encode(
        request,
        &call(Inputs::default(), max_output),
        compat,
        &texts(),
        &BTreeMap::new(),
    )
    .expect("不要 blob 的请求编码得了")
    .body
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
    // 没有工具，也没有工具调用：不发 tools。
    sample("plain-text", &body(&chat(), &Compat::default(), None));
}

#[test]
fn the_output_limit_and_usage_switches() {
    let compat = Compat {
        output_limit: OutputLimit::MaxCompletionTokens,
        stream_usage: false,
        ..Compat::default()
    };
    sample(
        "max-completion-tokens-without-usage",
        &body(&chat(), &compat, Some(4096)),
    );
}

#[test]
fn tool_calls_and_results() {
    let request = request(
        true,
        vec![
            user(vec![text("看看 src 目录")]),
            // 供应商自己的编号在私有数据里：线上用它，tool 消息也跟着它。
            assistant(vec![
                text("我先看一下。"),
                tool_call("call_2_1", "read", r#"{"path":"src"}"#, Some("call_abc")),
            ]),
            tool("call_2_1", vec![text("lib.rs\nmain.rs")]),
            // 没有供应商的编号：用内核的。空的、被截断的参数写成 {}。
            assistant(vec![
                tool_call("call_4_1", "read", "", None),
                tool_call("call_4_2", "read", r#"{"path":"src/li"#, None),
            ]),
            tool("call_4_1", vec![text("error: missing path")]),
            // 一个字都没回：写占位。
            tool("call_4_2", vec![]),
            assistant(vec![text("好了。")]),
        ],
    );
    sample(
        "tool-calls",
        &body(&request, &Compat::default(), Some(8192)),
    );
}

#[test]
fn user_blocks_are_joined_with_a_newline_only_where_needed() {
    let request = request(
        false,
        vec![
            // 事实块以换行结尾，照原样接上；两句话之间补一个换行，不粘在一起。
            user(vec![
                text("<env time=\"Fri 2026-09-25 17:00\" timezone=\"UTC+09:00\" cwd=\"~/src\"/>\n"),
                text("<permission level=\"workspace\"/>\n"),
                text("第一句"),
                text("第二句"),
            ]),
            assistant(vec![text("嗯")]),
            // 以换行结尾的不再补；空的一块什么都不接。
            user(vec![text("以换行结尾的\n"), text(""), text("下一句")]),
        ],
    );
    sample("user-blocks", &body(&request, &Compat::default(), None));
}

#[test]
fn an_empty_tool_face_is_sent_when_the_history_has_calls() {
    // 工具面是空的，可历史里调过工具：发 tools: []，有的网关要。
    let request = request(
        false,
        vec![
            user(vec![text("看看")]),
            assistant(vec![tool_call("call_2_1", "read", r#"{"path":"."}"#, None)]),
            tool("call_2_1", vec![text("Cargo.toml")]),
        ],
    );
    sample("empty-tools", &body(&request, &Compat::default(), None));
}

#[test]
fn blocks_it_does_not_know_are_skipped() {
    let unknown = miyu_kernel::block::Block::Unknown(raw(r#"{"type":"audio","seconds":3}"#));
    let request = request(false, vec![user(vec![text("听听这个"), unknown])]);
    sample("unknown-block", &body(&request, &Compat::default(), None));
}

#[test]
fn an_empty_system_is_not_sent() {
    let mut request = chat();
    request.system.clear();
    let body = String::from_utf8(body(&request, &Compat::default(), None)).unwrap();
    assert!(!body.contains("\"system\""), "{body}");
    assert!(body.starts_with(r#"{"model":"deepseek-v4","messages":[{"role":"user""#));
}

#[test]
fn each_message_is_found_where_it_was_written() {
    let request = chat();
    let encoded = encode(
        &request,
        &call(Inputs::default(), None),
        &Compat::default(),
        &texts(),
        &BTreeMap::new(),
    )
    .unwrap();
    let roles: Vec<String> = encoded
        .messages
        .iter()
        .map(|range| {
            let message: serde_json::Value = serde_json::from_slice(&encoded.body[range.clone()])
                .expect("每一段都是一条消息的 JSON");
            message["role"].as_str().unwrap().to_string()
        })
        .collect();
    assert_eq!(roles, ["system", "user", "assistant", "user"]);
}
