//! 内容块：消息和工具结果由它们组成（`docs/designs/03-事件模型.md` 第四节）。

use serde::{Deserialize, Deserializer, Serialize};

use crate::id::{CallId, ContentHash, DriverFamily, FileName, MediaType};
use crate::raw::{self, RawJson};

/// JSON 里用 `type` 分开五种；读到不认识的，整块原样留着，投影跳过它。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Block {
    Text(Text),
    Reasoning(Reasoning),
    Image(Image),
    File(File),
    ToolCall(ToolCall),
    #[serde(untagged)]
    Unknown(RawJson),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Text {
    pub text: String,
}

/// 模型的思考。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reasoning {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub private: Option<Private>,
}

/// 图片。宽和高在进来的时候就量好了；量不出尺寸的不当图片，当文件。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Image {
    pub blob: ContentHash,
    pub media_type: MediaType,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct File {
    pub blob: ContentHash,
    pub name: FileName,
    pub media_type: MediaType,
}

/// 一次工具调用。工具名和参数不检查：模型说了什么就记什么，
/// 名字不对、参数坏了，是执行时报给模型的错。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    pub call_id: CallId,
    pub name: String,
    /// 模型给出的参数原文，不解析后重新写：重新写会改变字节，前缀缓存随之失效。
    pub args: String,
    /// 供应商自己的调用编号放在这里。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub private: Option<Private>,
}

/// 驱动私有的原样数据。内核不解读，同一类驱动发请求时原样带上，别的驱动不理它
/// （`docs/designs/03-事件模型.md` 第九节）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Private {
    pub driver: DriverFamily,
    pub data: RawJson,
}

impl<'de> Deserialize<'de> for Block {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        raw::read_tagged(
            d,
            "type",
            |kind, json| {
                Some(match kind {
                    "text" => raw::parse(json).map(Block::Text),
                    "reasoning" => raw::parse(json).map(Block::Reasoning),
                    "image" => raw::parse(json).map(Block::Image),
                    "file" => raw::parse(json).map(Block::File),
                    "tool_call" => raw::parse(json).map(Block::ToolCall),
                    _ => return None,
                })
            },
            Block::Unknown,
        )
    }
}

#[cfg(test)]
mod tests;
