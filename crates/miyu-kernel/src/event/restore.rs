//! 改回文件的结局（`docs/designs/03-事件模型.md` 第三节、`10-自带软件.md` 第七节「改回文件的细则」，施工 4-7 上）：
//! 撤销、恢复时照效果改回文件，一步一项记下照哪个效果、做了什么、成了没有。

use serde::{Deserialize, Serialize};

use crate::id::{ContentHash, Seq};
use crate::text_enum::text_enum;

/// `files.restored`：撤销、恢复时改回文件的结局，照做的先后。它不属于哪一轮，不进上下文；`cause` 是撤销、恢复的
/// 那个命令。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilesRestored {
    /// 每一步。
    pub files: Vec<Restored>,
}

/// 改回的一步。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Restored {
    /// 照哪一条 `tool.result`：它的序号。
    pub result: Seq,
    /// 照那一条的第几个效果，从 0 数起。
    pub effect: u32,
    /// 改的是哪里：效果里记的那个路径。
    pub path: String,
    /// 做了什么。
    pub action: RestoreAction,
    /// 结局。
    pub outcome: RestoreOutcome,
    /// 内容被改过（`changed`）时，现在的内容的哈希。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub found: Option<ContentHash>,
    /// 移进了回收站（`trash` 成了）时，它在回收站里的新位置。下一次撤销照它移回来。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trash: Option<String>,
    /// 移回来的是一个文件（`untrash` 成了）时，它的内容的哈希。恢复时照它核对，再移进回收站。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<ContentHash>,
    /// 出错（`failed`）时，系统的原话。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

text_enum!(
    /// 改回的一步做了什么。
    RestoreAction {
        /// 写回一份内容。
        Write = "write",
        /// 移进回收站。
        Trash = "trash",
        /// 从回收站移回原处。
        Untrash = "untrash",
    }
);

text_enum!(
    /// 改回的一步的结局。除了 `restored`，都是没动。
    RestoreOutcome {
        /// 改回了，或者现在已经是要改成的样子。
        Restored = "restored",
        /// 内容被改过了：不是她留下的样子。
        Changed = "changed",
        /// 东西没了。
        Missing = "missing",
        /// 原处被占了。
        Occupied = "occupied",
        /// 回收站里已经没有了。
        Gone = "gone",
        /// 要写回的内容当时没存下来。
        Unsaved = "unsaved",
        /// 回收站收不了：不删（`10-自带软件.md` 第三节）。
        Unavailable = "unavailable",
        /// 出错了。
        Failed = "failed",
    }
);

#[cfg(test)]
mod tests;
