//! 改标题、置顶（施工 3-8 三补，`docs/blueprint/kernel/session.md`「改标题、置顶」）：人改，内核记；和现在一样的不记，
//! 只写改了的那几格。会话现在的标题、置顶从日志里的 `session.meta_changed` 一路算，载入时照样算回来。

use super::action::Action;
use super::{Session, accepted};
use crate::event::{Body, CommandRan, MetaChanged};
use crate::id::CommandId;
use crate::origin::By;
use crate::time::Timestamp;

/// 会话现在的标题、置顶：日志里的 `session.meta_changed` 一条条盖上去的结果。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Meta {
    /// 标题；没起过、去掉了的是空的。
    title: String,
    /// 置顶了没有；没改过的是没有。
    pinned: bool,
}

impl Meta {
    /// 盖上一条 `session.meta_changed`：写了的格换成它的，没写的照旧。标题写空的是去掉标题。
    pub(super) fn note(&mut self, changed: &MetaChanged) {
        if let Some(title) = &changed.title {
            self.title.clone_from(title);
        }
        if let Some(pinned) = changed.pinned {
            self.pinned = pinned;
        }
    }
}

impl Session {
    /// 改标题、置顶：和现在一样的格去掉，还剩的记一条 `session.meta_changed`，回合进行中的带上这个回合；一格都不剩的，
    /// 接受，什么都不记，编号照样记下。改了什么不影响回合：标题、置顶不进请求。
    pub(super) fn set_meta(
        &mut self,
        id: CommandId,
        by: By,
        at: Timestamp,
        title: Option<String>,
        pinned: Option<bool>,
    ) -> Vec<Action> {
        let changed = MetaChanged {
            title: title.filter(|title| *title != self.meta.title),
            pinned: pinned.filter(|pinned| *pinned != self.meta.pinned),
        };
        if changed.title.is_none() && changed.pinned.is_none() {
            self.recent.insert(id.clone(), Vec::new());
            return vec![accepted(id, Vec::new())];
        }
        self.meta.note(&changed);
        let event = self.record(at, by, Some(id.clone()), Body::MetaChanged(changed));
        self.accept(id, vec![event.seq]);
        vec![Action::Append(vec![event])]
    }

    /// 记下人用了一个斜杠命令（施工 O-6）：记一条 `command.ran`，回合进行中的带上这个回合；落了盘回应，附上这一条。不影响回合：
    /// 它不进请求。
    pub(super) fn ran(
        &mut self,
        id: CommandId,
        by: By,
        at: Timestamp,
        text: String,
        command: String,
    ) -> Vec<Action> {
        let body = Body::CommandRan(CommandRan { text, command });
        let event = self.record(at, by, Some(id.clone()), body);
        self.accept(id, vec![event.seq]);
        vec![Action::Append(vec![event])]
    }
}
