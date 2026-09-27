//! 效果（`docs/designs/10-自带软件.md` 第五节、`03-事件模型.md` 第三节，施工 4-6 上）：随 `tool.result` 记进
//! 日志，给内核和头看，不发给模型。diff、撤销、改之前的核对、压缩后的工作集都照它们算。
//!
//! 认识的三种读成对应的类型；不认识的（第三方的工具报来的）整块原样留着，内核不解读。

use serde::{Deserialize, Deserializer, Serialize};

use crate::id::ContentHash;
use crate::raw::{self, RawJson};

/// 一样效果，照 `kind` 分。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind")]
pub enum Effect {
    /// 读了一个文件。
    #[serde(rename = "file.read")]
    FileRead(FileRead),
    /// 改了一个文件：新建、覆盖、编辑。
    #[serde(rename = "file.changed")]
    FileChanged(FileChanged),
    /// 把一个文件移进了回收站。
    #[serde(rename = "file.trashed")]
    FileTrashed(FileTrashed),
    /// 不认识的种类：整块原样留着，写出去还是原样。
    #[serde(untagged)]
    Unknown(RawJson),
}

/// `file.read`：读了一个文件。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileRead {
    /// 换成真实位置以后的绝对路径。
    pub path: String,
    /// 读了第几行到第几行：从 1 数起，含两头。一行都没显示的（空文件、过了结尾）没有这一格。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lines: Option<[u64; 2]>,
    /// 读的时候整份文件的内容哈希，读了一段的也是整份的：改之前照它核对。
    pub hash: ContentHash,
}

/// `file.changed`：改了一个文件。改前改后的内容都存成了 blob，这里是它们的哈希。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileChanged {
    /// 换成真实位置以后的绝对路径。
    pub path: String,
    /// 改前的内容。新建的没有，写成 `null`。
    #[serde(default)]
    pub before: Option<ContentHash>,
    /// 改后的内容。
    pub after: ContentHash,
}

/// `file.trashed`：把一个文件移进了回收站（施工 4-6 下才有工具报它）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileTrashed {
    /// 移走之前的位置：换成真实位置以后的绝对路径。
    pub path: String,
    /// 回收站里的位置：各平台自己的写法，撤销时照它移回来。
    pub trash: String,
}

impl<'de> Deserialize<'de> for Effect {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        raw::read_tagged(
            d,
            "kind",
            |kind, json| {
                Some(match kind {
                    "file.read" => raw::parse(json).map(Effect::FileRead),
                    "file.changed" => raw::parse(json).map(Effect::FileChanged),
                    "file.trashed" => raw::parse(json).map(Effect::FileTrashed),
                    _ => return None,
                })
            },
            Effect::Unknown,
        )
    }
}

#[cfg(test)]
mod tests;
