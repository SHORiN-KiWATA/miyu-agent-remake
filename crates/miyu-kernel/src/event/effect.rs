//! 效果（`docs/designs/10-自带软件.md` 第五节、`03-事件模型.md` 第三节，施工 4-6 上）：随 `tool.result` 记进
//! 日志，给内核和头看，不发给模型。diff、撤销、改之前的核对、压缩后的工作集都照它们算。
//!
//! 认识的六种读成对应的类型；不认识的（第三方的工具报来的）整块原样留着，内核不解读。`job.started` 是派出去一个
//! 任务的记录（施工 7-1，`agents.md`）：派它的那次调用本身就在历史里，不另记事件。`job.messaged` 是给子代理留了言的
//! 记录（施工 7-7）：它欠一份回报。`peer.watch` 是订了「空了告诉我」的记录（施工 C-1，`cross-session.md`）：账本照它算在
//! 等哪几个会话。`todo.written` 是 `todowrite` 换上的整份待办（施工 D-3）：有效历史照它算当前的清单。

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
    /// 订了别的会话的「空了告诉我」（施工 C-1）。
    #[serde(rename = "peer.watch")]
    PeerWatch(PeerWatch),
    /// 换上了一整份待办（施工 D-3）。
    #[serde(rename = "todo.written")]
    TodoWritten(TodoWritten),
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

/// `peer.watch`：`send_message` 订了「空了告诉我」，那次调用报一条（施工 C-1，`cross-session.md`「效果 peer.watch」）。
/// 这就是订的记录，不另记事件：账本照它算在等哪几个会话、从这条结果的时刻算起；订的不是这个会话自己，也由账本查
/// （`kernel/history.md`）。被等的那一边不记。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerWatch {
    /// 被等的会话的整个编号。
    pub session: SessionId,
}

/// `todo.written`：`todowrite` 换上的整份待办（施工 D-3，`docs/blueprint/tools/todowrite.md`）。全部做完的清空，是空列表。
/// 有效历史照最近一份没撤掉的算当前的清单（`history/todos.rs`），交给头、压缩时写进检查点。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TodoWritten {
    /// 照先后的每一项。
    pub todos: Vec<Todo>,
    /// 因为全部做完而清空的（施工 D-3 补）：清空前那一份，每一项都做完了。头照它让人看到最后一项打勾。别的写法是空的，不写。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub done: Vec<Todo>,
}

/// 待办的一项。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Todo {
    /// 一句话：要做的事。
    pub content: String,
    /// 做到哪了。
    pub status: TodoStatus,
}

text_enum!(
    /// 待办一项做到哪了（施工 D-3）。不认识的是新版本才有的，原样留着。
    TodoStatus {
        /// 还没开始。
        Pending = "pending",
        /// 正在做。
        InProgress = "in_progress",
        /// 做完了。
        Completed = "completed",
    }
);

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
                    "peer.watch" => raw::parse(json).map(Effect::PeerWatch),
                    "todo.written" => raw::parse(json).map(Effect::TodoWritten),
                    _ => return None,
                })
            },
            Effect::Unknown,
        )
    }
}

#[cfg(test)]
mod tests;
