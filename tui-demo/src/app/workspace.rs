//! 换工作区（蓝图 `tui.md`「新会话：人格、工作区」第 4 条，核心 9-7 下）：`/workspace <路径>` 开着的会话交给核心的
//! `command.run`，还没开的先验目录、记着等造会话；不带路径的弹「请指定路径」。

use super::App;
use crate::core::{Command, Refusal};

impl App {
    /// `/workspace`：带路径的换过去，不带的弹「请指定路径」（2026-10-10 项目主人）。
    pub(super) fn workspace_command(&mut self, words: &str) {
        match words.trim() {
            "" => {
                let need = self.config.text.workspace.need_path.clone();
                self.hint(need, false);
            }
            path => self.change_workspace(path.to_string()),
        }
    }

    /// 现在用的工作区：开着的会话照会话的，还没开的照换过的，没换的照终端所在的。
    pub fn current_workspace(&self) -> &str {
        match &self.transcript.session {
            Some(_) => self.transcript.workspace.as_deref().unwrap_or(&self.cwd),
            None => self.workspace_pending.as_deref().unwrap_or(&self.cwd),
        }
    }

    /// 换到 `path`（相对的照终端所在的目录接）：开着的会话交给核心，还没开的先验。
    fn change_workspace(&mut self, path: String) {
        if !self.reachable() {
            return;
        }
        let cwd = self.cwd.clone();
        let command = if self.transcript.session.is_some() {
            Command::Workspace { path, cwd }
        } else {
            Command::CheckDir { path, cwd }
        };
        self.core.send(command);
    }

    /// 还没开会话时验完了：成了的记着、告诉核心造会话时带上，弹「新会话开在 …」；不成的照原因弹提示。
    pub(super) fn dir_checked(&mut self, result: Result<String, Refusal>) {
        match result {
            Ok(dir) => {
                self.core.send(Command::NewWorkspace(Some(dir.clone())));
                let note = (self.config.text.workspace.pending)
                    .replace("{path}", &crate::local::home_short(&dir));
                self.workspace_pending = Some(dir);
                self.hint(note, true);
            }
            Err(refusal) => {
                let hints = &self.config.text.refusal_hints;
                let hint = refusal.reason.and_then(|r| hints.get(&r).cloned());
                self.hint(hint.unwrap_or(refusal.message), false);
            }
        }
    }

    /// `/new` 以后回到终端所在的目录。
    pub(super) fn forget_workspace(&mut self) {
        self.workspace_pending = None;
    }
}
