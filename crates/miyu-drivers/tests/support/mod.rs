//! 驱动测试用的：出厂的占位、造块、造一次调用、和样本逐字节比对。
//!
//! 样本在 `docs/designs/samples/drivers/openai-chat/`，一种写法一个文件，写的是请求字节，末尾一个
//! 换行。字节变了必须是有意的：设上 `MIYU_PROBE_WRITE=1` 跑一遍，重写样本，提交说明里写为什么变。

#![allow(dead_code, reason = "几个测试文件各用其中一部分")]

use std::fs;
use std::path::PathBuf;

use miyu_drivers::openai_chat::{Compat, ReasoningField, ReasoningReplay};
use miyu_drivers::{Call, DriverTextSources, DriverTexts, Inputs};
use miyu_kernel::block::{Block, File, Image, Private, Reasoning, Text, ToolCall};
use miyu_kernel::id::{CallId, ContentHash, DriverFamily, FileName, MediaType, ModelName};
use miyu_kernel::raw::RawJson;
use miyu_kernel::request::ToolSpec;

/// 出厂的占位，从资源目录读。
pub fn texts() -> DriverTexts {
    DriverTexts::new(DriverTextSources {
        image_omitted: include_str!("../../../../resources/core/drivers/image-omitted.txt"),
        file_omitted: include_str!("../../../../resources/core/drivers/file-omitted.txt"),
        no_output: include_str!("../../../../resources/core/drivers/no-output.txt"),
        tool_attachments: include_str!("../../../../resources/core/drivers/tool-attachments.txt"),
        tool_attachments_only: include_str!(
            "../../../../resources/core/drivers/tool-attachments-only.txt"
        ),
    })
    .expect("出厂的占位用得了")
}

/// 发给 `deepseek-v4`，能收哪些输入照 `inputs`。
pub fn call(inputs: Inputs, max_output: Option<u32>) -> Call {
    Call {
        model: ModelName::parse("deepseek-v4").expect("模型名合写法"),
        max_output,
        inputs,
    }
}

/// 能看图、能读 PDF。
pub fn sees_all() -> Inputs {
    Inputs {
        images: true,
        pdf: true,
    }
}

/// DeepSeek 那一套：每条 assistant 都带 `reasoning_content`，没有就发空串。
pub fn deepseek() -> Compat {
    Compat {
        reasoning: ReasoningReplay::Replay {
            field: ReasoningField::ReasoningContent,
            always: true,
        },
        ..Compat::default()
    }
}

pub fn text(text: &str) -> Block {
    Block::Text(Text {
        text: text.to_string(),
    })
}

pub fn thought(text: &str) -> Block {
    Block::Reasoning(Reasoning {
        text: text.to_string(),
        private: None,
    })
}

/// 一次工具调用；`provider` 是供应商自己的编号，记在这个驱动的私有数据里。
pub fn tool_call(call_id: &str, name: &str, args: &str, provider: Option<&str>) -> Block {
    Block::ToolCall(ToolCall {
        call_id: id(call_id),
        name: name.to_string(),
        args: args.to_string(),
        private: provider.map(|provider| Private {
            driver: DriverFamily::parse("openai-chat").expect("驱动家族合写法"),
            data: raw(&format!(r#"{{"id":"{provider}"}}"#)),
        }),
    })
}

pub fn image(content: &[u8], media_type: &str) -> Block {
    Block::Image(Image {
        blob: ContentHash::of(content),
        media_type: MediaType::parse(media_type).expect("媒体类型合写法"),
        width: 800,
        height: 600,
    })
}

pub fn file(content: &[u8], name: &str, media_type: &str) -> Block {
    Block::File(File {
        blob: ContentHash::of(content),
        name: FileName::parse(name).expect("文件名合写法"),
        media_type: MediaType::parse(media_type).expect("媒体类型合写法"),
    })
}

pub fn id(call_id: &str) -> CallId {
    CallId::parse(call_id).expect("调用编号合写法")
}

pub fn raw(json: &str) -> RawJson {
    serde_json::from_str(json).expect("是 JSON")
}

/// 工具面上的 `read`。
pub fn read_tool() -> ToolSpec {
    ToolSpec {
        name: "read".to_string(),
        description: "Read a text file.".to_string(),
        parameters: raw(
            r#"{"type":"object","properties":{"path":{"type":"string"}},"required":["path"]}"#,
        ),
    }
}

/// 和样本逐字节比对；设上 `MIYU_PROBE_WRITE=1` 时重写样本。
pub fn sample(name: &str, body: &[u8]) {
    let mut content = body.to_vec();
    content.push(b'\n');
    sample_file(&format!("{name}.json"), &content);
}

/// 样本目录下的一个文件，和 `content` 逐字节比对；设上 `MIYU_PROBE_WRITE=1` 时重写它。
pub fn sample_file(name: &str, content: &[u8]) {
    let path = dir().join(name);
    if std::env::var_os("MIYU_PROBE_WRITE").is_some() {
        fs::create_dir_all(path.parent().expect("样本在样本目录里")).expect("建得了样本目录");
        fs::write(&path, content).expect("写得了样本");
        return;
    }
    let archived = fs::read(&path).unwrap_or_else(|e| panic!("读不了 {}：{e}", path.display()));
    assert!(
        archived == content,
        "{name} 和样本不一样。要是有意改的，设上 MIYU_PROBE_WRITE=1 跑一遍重写样本，提交说明里写为什么变\n样本：{}\n这次：{}",
        String::from_utf8_lossy(&archived),
        String::from_utf8_lossy(content)
    );
}

/// 样本目录：这个 crate 的目录往上两级是仓库根。
pub fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/designs/samples/drivers/openai-chat")
}
