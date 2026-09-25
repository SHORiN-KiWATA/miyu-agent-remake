//! 内容块：消息和工具结果由它们组成（`docs/designs/03-事件模型.md` 第四节）。
//!
//! 图片、文件这些大内容不放进事件，按内容哈希存成 blob，块里只放引用（03 E4）。

use serde::{Deserialize, Deserializer, Serialize};

use crate::id::{CallId, ContentHash, DriverFamily, FileName, MediaType};
use crate::raw::{self, RawJson};

/// 一块内容。JSON 里用 `type` 分开五种；读到不认识的，整块原样留着，投影跳过它。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Block {
    /// 文本。
    Text(Text),
    /// 模型的思考。
    Reasoning(Reasoning),
    /// 图片。
    Image(Image),
    /// 文件，例如 PDF。
    File(File),
    /// 模型发起的一次工具调用。
    ToolCall(ToolCall),
    /// 不认识的种类，新版本才有的：整块原样留着，写出去还是原样。
    #[serde(untagged)]
    Unknown(RawJson),
}

/// 文本。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Text {
    /// 文字。
    pub text: String,
}

/// 模型的思考。投影保留全部思考块，发哪些由驱动按供应商的要求决定（`08-上下文投影.md` 第四节）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reasoning {
    /// 思考的文字。
    pub text: String,
    /// 供应商要原样传回的数据，例如思考块的签名。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub private: Option<Private>,
}

/// 图片。宽和高在进来的时候就量好了；量不出尺寸的不当图片，当文件。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Image {
    /// 图片本身存成的 blob。
    pub blob: ContentHash,
    /// 例如 `image/png`。
    pub media_type: MediaType,
    /// 宽，像素。
    pub width: u32,
    /// 高，像素。
    pub height: u32,
}

/// 文件。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct File {
    /// 文件本身存成的 blob。
    pub blob: ContentHash,
    /// 文件名，例如 `报告.pdf`。只是名字，不带路径。
    pub name: FileName,
    /// 例如 `application/pdf`。
    pub media_type: MediaType,
}

/// 一次工具调用。工具名和参数不检查：模型说了什么就记什么，
/// 名字不对、参数坏了，是执行时报给模型的错。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    /// 内核分配的调用编号；供应商自己的编号在 `private` 里。
    pub call_id: CallId,
    /// 模型说要调用的工具名。
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
    /// 哪一类驱动的数据。同一类驱动发请求时原样带上，别的驱动不理它。
    pub driver: DriverFamily,
    /// 数据本身，任意 JSON，一个字节都不改。
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
