//! 会话里换模型（施工 8-10，`docs/blueprint/models.md`「怎么走」第六条，`kernel/session.md`「换模型」）：人换，内核记；
//! 和现在的一样的不记，下一个回合开始时生效。
//!
//! 会话的引用从日志算：`session.created` 的 `model`，被后来带 `model` 的 `session.policy_changed` 盖掉，撤掉的回合里的也算
//! （换模型不是对话的一部分，照改标题）。内核只存字，不解读：回合开始时交给执行器照这一轮的配置重新解析，钉着的没了、
//! 执行器退回了默认的，内核在注入前面记一条，`by` 是内核。

use super::action::Action;
use super::input::Replaced;
use super::{Session, accepted};
use crate::event::{Body, Event, PolicyChanged};
use crate::id::{CommandId, Seq};
use crate::origin::By;
use crate::time::Timestamp;

/// 会话的引用，和最近一次换模型写在第几条：每追加一条记一次，载入时照整份日志一路算（`load.rs`）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Reference {
    /// 会话现在的引用；以前的日志没有记下的、造的时候连 `models.chat` 都没配的没有。
    model: Option<String>,
    /// 最近一条带 `model` 的 `session.policy_changed` 的序号：写在它前面的自动压缩的暂停、失败不再算（`compaction.md`
    /// 第十条第 6 条，「施工时定的」8-10）。没换过的没有。
    changed: Option<Seq>,
}

impl Reference {
    /// 记下一条：`session.created` 定开头的引用，带 `model` 的 `session.policy_changed` 换掉它。
    pub(super) fn note(&mut self, event: &Event) {
        match &event.body {
            Body::SessionCreated(created) => self.model.clone_from(&created.model),
            Body::PolicyChanged(PolicyChanged {
                model: Some(model), ..
            }) => {
                self.model = Some(model.clone());
                self.changed = Some(event.seq);
            }
            _ => {}
        }
    }

    /// 会话现在的引用。
    pub(super) fn current(&self) -> Option<&str> {
        self.model.as_deref()
    }

    /// 第 `seq` 条写在最近一次换模型后面；没换过的都算。熔断照它只看换过去以后的暂停、失败。
    pub(super) fn after_change(&self, seq: Seq) -> bool {
        self.changed.is_none_or(|changed| seq > changed)
    }
}

impl Session {
    /// 会话现在的引用：模型或 `@池`（施工 8-10）。`session.created` 的 `model`，被后来带 `model` 的 `session.policy_changed`
    /// 盖掉，撤掉的回合里的也算；以前的日志没有的、造的时候连 `models.chat` 都没配的没有（跟着 `models.chat`）。只读：执行器
    /// 载入时照它造路由（`session/actor.md` 第 2 条）。
    pub fn reference(&self) -> Option<&str> {
        self.reference.current()
    }

    /// 换模型：和现在的一样的接受，什么都不记，编号照样记下；不一样的记一条 `session.policy_changed`，`by` 是换的人，回合
    /// 进行中的带上这个回合，空闲时没有。什么时候来都收，改回文件的时候照「命令和回应」第 5 条拒（`session.rs`）。
    pub(super) fn configure(
        &mut self,
        id: CommandId,
        by: By,
        at: Timestamp,
        model: String,
    ) -> Vec<Action> {
        if self.reference() == Some(model.as_str()) {
            self.recent.insert(id.clone(), Vec::new());
            return vec![accepted(id, Vec::new())];
        }
        let body = Body::PolicyChanged(PolicyChanged {
            model: Some(model),
            ..PolicyChanged::default()
        });
        let event = self.record(at, by, Some(id.clone()), body);
        self.accept(id, vec![event.seq]);
        vec![Action::Append(vec![event])]
    }

    /// 回合开始时执行器退回了默认（施工 8-10）：`from` 正是现在的引用、`to` 和它不一样的，记一条 `session.policy_changed`
    /// （`model` 是退回的，`replaced` 是原来的），`by` 是内核，`cause` 是回合的。对不上的不理：执行器交回来之前人又换了，
    /// 人换的算数，下一轮再解析（「施工时定的」8-10）。
    pub(super) fn fall_back(
        &mut self,
        at: Timestamp,
        cause: Option<CommandId>,
        replaced: Option<Replaced>,
    ) -> Option<Event> {
        let Replaced { from, to } = replaced?;
        if self.reference() != Some(from.as_str()) || from == to {
            return None;
        }
        let body = Body::PolicyChanged(PolicyChanged {
            model: Some(to),
            replaced: Some(from),
            ..PolicyChanged::default()
        });
        Some(self.record(at, By::Kernel, cause, body))
    }
}
