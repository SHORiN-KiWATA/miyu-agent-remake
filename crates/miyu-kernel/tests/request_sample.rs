//! 样本请求（`docs/designs/samples/requests/second-step.json`，`08-上下文投影.md` 第二节）：
//! 样本会话里 42 号回合的第二次请求，也就是工具结果回来以后那一次。在代码里造出同一份请求，
//! 规范的字节要和样本文件一字不差；请求的哈希就是这串字节的 SHA-256。

use std::fs;
use std::path::PathBuf;

use miyu_kernel::block::{Block, Text, ToolCall};
use miyu_kernel::id::{CallId, ContentHash};
use miyu_kernel::raw::RawJson;
use miyu_kernel::request::{Message, Request, ToolSpec};

const READ: &str = "Read a text file by line pages, an image, a PDF, or list a directory. Prefer this over `cat` in the shell: files read here come back after compaction.";
const READ_PARAMETERS: &str = r#"{"type":"object","properties":{"path":{"type":"string"},"offset":{"type":"integer"},"limit":{"type":"integer"}},"required":["path"]}"#;
/// 43、44 号注入的两块，照模板，行尾都有一个换行。
const ENV: &str =
    "<env time=\"Fri 2026-09-25 16:00\" timezone=\"UTC+09:00\" cwd=\"~/src/miyu\"/>\n";
const PERMISSION: &str = "<permission level=\"workspace\"/>\n";

fn text(text: &str) -> Block {
    Block::Text(Text {
        text: text.to_string(),
    })
}

/// 样本会话里，47 号工具结果回来以后的那一次请求。
fn second_step() -> Request {
    let call = CallId::parse("call_45_1").expect("样本里的调用编号合写法");
    let parameters: RawJson = serde_json::from_str(READ_PARAMETERS).expect("参数格式是 JSON");
    Request {
        tools: vec![ToolSpec {
            name: "read".to_string(),
            description: READ.to_string(),
            parameters,
        }],
        system: "You are a helpful software engineer.".to_string(),
        messages: vec![
            // 43、44 号注入的环境和权限，排在 41 号触发消息的前面（08 C2）。
            Message::User {
                blocks: vec![text(ENV), text(PERMISSION), text("看看 src 目录")],
            },
            // 45 号：模型的回复，和它的工具调用。
            Message::Assistant {
                blocks: vec![
                    text("我先看一下目录。"),
                    Block::ToolCall(ToolCall {
                        call_id: call,
                        name: "read".to_string(),
                        args: r#"{"path":"src"}"#.to_string(),
                        private: None,
                    }),
                ],
            },
            // 47 号：工具的结果。46 号是第一次请求的 model.called，不进上下文。
            Message::Tool {
                call_id: call,
                error: false,
                blocks: vec![text("lib.rs\nmain.rs")],
            },
        ],
        stable: 0,
    }
}

/// 样本文件的内容，去掉行尾的换行。
fn sample() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/designs/samples/requests/second-step.json");
    let text =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("读不了 {}：{e}", path.display()));
    text.strip_suffix('\n')
        .unwrap_or_else(|| panic!("样本请求要以一个换行结尾"))
        .to_string()
}

#[test]
fn the_sample_is_the_canonical_bytes_of_the_second_step() {
    let bytes = String::from_utf8(second_step().canonical_bytes()).expect("规范的字节是 UTF-8");
    assert_eq!(bytes, sample());
}

#[test]
fn the_request_hash_is_the_sha256_of_the_sample() {
    assert_eq!(second_step().hash(), ContentHash::of(sample().as_bytes()));
}
