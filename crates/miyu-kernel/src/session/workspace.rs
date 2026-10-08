//! 换会话在哪个目录干活（施工 9-7 上，`docs/blueprint/kernel/session.md`「换工作区」）：人换，内核记；和现在一样的不记。工作区
//! 是会话的属性，会话的环境当场换，这一轮里照旧用开轮时取的那一份（开回合时取，`turn.rs`），下一轮开始照新的，变了的事实照旧
//! 只注入变了的。

use super::action::Action;
use super::{Session, accepted};
use crate::event::{Body, WorkspaceChanged};
use crate::id::CommandId;
use crate::origin::By;
use crate::time::Timestamp;

impl Session {
    /// 会话现在加进来的目录（施工 9-7 上）：协议照它回 `subscribe` 的 `workspace`。工作目录是 [`Session::cwd`]。
    pub fn dirs(&self) -> &[String] {
        &self.environment.dirs
    }

    /// 换工作区：工作目录换成 `cwd`，写了 `dirs` 的加进来的目录整份换掉。和现在一样的，接受，什么都不记，编号照样记下；
    /// 变了的记一条 `session.workspace_changed`（`dirs` 没变的不写），回合进行中的带上这个回合，落了盘回应。
    pub(super) fn set_workspace(
        &mut self,
        id: CommandId,
        by: By,
        at: Timestamp,
        cwd: String,
        dirs: Option<Vec<String>>,
    ) -> Vec<Action> {
        let dirs = dirs.filter(|dirs| *dirs != self.environment.dirs);
        if cwd == self.environment.cwd && dirs.is_none() {
            self.recent.insert(id.clone(), Vec::new());
            return vec![accepted(id, Vec::new())];
        }
        self.environment.cwd.clone_from(&cwd);
        if let Some(dirs) = &dirs {
            self.environment.dirs.clone_from(dirs);
        }
        let body = Body::WorkspaceChanged(WorkspaceChanged { cwd, dirs });
        let event = self.record(at, by, Some(id.clone()), body);
        self.accept(id, vec![event.seq]);
        vec![Action::Append(vec![event])]
    }
}
