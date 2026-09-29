//! 效果（`docs/designs/10-自带软件.md` 第五节、`03-事件模型.md` 第三节，施工 4-6 上）：随 `tool.result` 记进
//! 日志，给内核和头看，不发给模型。diff、撤销、改之前的核对、压缩后的工作集都照它们算。
//!
//! 认识的五种读成对应的类型；不认识的（第三方的工具报来的）整块原样留着，内核不解读。`job.started` 是派出去一个
//! 任务的记录（施工 7-1，`agents.md`）：派它的那次调用本身就在历史里，不另记事件。`job.messaged` 是给子代理留了言的
//! 记录（施工 7-7）：它欠一份回报。

use serde::{Deserialize, Deserializer, Serialize};

use crate::id::{ContentHash, JobId, SessionId};
use crate::raw::{self, RawJson};
use crate::text_enum::text_enum;

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
    /// 派出去一个任务：后台命令，或者子代理（施工 7-1）。
    #[serde(rename = "job.started")]
    JobStarted(JobStarted),
    /// 给自己派的一个子代理留了言（施工 7-7）：它欠一份回报。
    #[serde(rename = "job.messaged")]
    JobMessaged(JobMessaged),
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

/// `job.started`：派出去一个任务（`agents.md`「对外的样子」）。编号整份日志里不重复、`agent` 带会话、`command` 不带，
/// 由账本查（`kernel/history.md`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobStarted {
    /// 任务编号：一个会话里从 1 数起，后台命令和子代理共用一串，不回收；子会话派的带上它在父会话里的编号（`j2.1`，施工
    /// 7-1 补）。
    pub job: JobId,
    /// 派的是什么。
    pub what: JobKind,
    /// 调用时给的 `description`，头显示用。
    pub title: String,
    /// 子代理的会话；后台命令没有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<SessionId>,
}

/// `job.messaged`：给自己派的子代理留了言（施工 7-7，`agents.md` 第六条）。子代理收到父会话的留言就欠一份回报，父会话
/// 照它等：派了孙代理的子会话，给孙代理留了言、孙代理还没报的，先不向上回报（`agents.md` 第二条第 2 条）。账本查它对得上
/// 这个会话派的一个子代理（`kernel/history.md`）。发给父会话的留言不报它：父会话不欠谁的。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobMessaged {
    /// 留言给了哪个子代理：它的任务编号。
    pub job: JobId,
}

text_enum!(
    /// 派出去的任务是什么。不认识的是新版本才有的，账本不查它带不带会话，两种回报都对不上它。
    JobKind {
        /// 后台命令：`shell` 的 `run_in_background`。
        Command = "command",
        /// 子代理：一个子会话。
        Agent = "agent",
    }
);

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
                    "job.started" => raw::parse(json).map(Effect::JobStarted),
                    "job.messaged" => raw::parse(json).map(Effect::JobMessaged),
                    _ => return None,
                })
            },
            Effect::Unknown,
        )
    }
}

#[cfg(test)]
mod tests;
