//! 会话的工作区（蓝图 `tui.md`「新会话：人格、工作区」第 5 条，核心 9-7 上）：工作区是会话自己记着的，订阅的回应里给、
//! 换了推 `session.workspace_changed`。侧边栏「工作目录」照它写；推来换了的正文写一行，接上老会话时终端在别的目录的
//! 写一句她在哪干活。

use serde::Deserialize;

use super::{Kind, Transcript};
use crate::config::Texts as AllTexts;
use crate::local::home_short;

/// 工作区那几句的字（`text/<语言>.json` 的 `workspace`）。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Texts {
    /// `/workspace` 没带路径。
    pub need_path: String,
    /// 还没开会话时换成了：`{path}` 是验过的目录。
    pub pending: String,
    /// 接上老会话、终端在别的目录：正文末尾那一句，`{path}` 是会话的工作区（核心 9-7 上）。
    pub elsewhere: String,
    /// 推来换了工作区：正文那一行，`{path}` 是换到的目录。
    pub changed: String,
}

impl Transcript {
    /// 推来换了工作区（哪个头换的都算）：记下，正文写一行暗色的「工作区换到 <目录>」。
    pub(super) fn workspace_changed(&mut self, cwd: String, texts: &AllTexts) {
        let line = texts.workspace.changed.replace("{path}", &home_short(&cwd));
        self.workspace = Some(cwd);
        self.note(Kind::Note, line);
    }

    /// 订阅回应里的工作区：记下。`joined` 是切过去、启动时接上老会话的那一次：终端所在的目录（`here`）和它不一样的，
    /// 正文末尾写一句「她在 <目录> 干活」，一个会话只写一次（掉了队重新订阅不再写）。
    pub fn joined_workspace(&mut self, cwd: String, joined: bool, here: &str, texts: &AllTexts) {
        if joined && !self.workspace_told && cwd != here {
            let line = texts
                .workspace
                .elsewhere
                .replace("{path}", &home_short(&cwd));
            self.note(Kind::Note, line);
        }
        self.workspace_told |= joined;
        self.workspace = Some(cwd);
    }
}
