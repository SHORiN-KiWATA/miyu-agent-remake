//! 切权限级别（`docs/designs/02-内核.md` 第六节「权限级别怎么切」）：人切，内核记；收紧当场生效，
//! 放宽等下一次请求；只读时拦写文件的调用；请求之前把事实查一遍，变了的才注入。

use super::action::{Action, Reason};
use super::turn::Stage;
use super::{Session, accepted, rejected};
use crate::event::{Body, Event, Level, Permission, PolicyChanged};
use crate::facts::{Environment, changed};
use crate::id::CommandId;
use crate::origin::By;
use crate::time::Timestamp;

impl Session {
    /// 切权限级别：照现在的合出新的，记一条 `session.policy_changed`。收紧的当场生效，收紧成
    /// 只读的，这一步里还没派的写文件调用当场拦下；放宽的等下一次请求。回合进行中切的，下一次
    /// 请求之前把事实查一遍，这时正要请求的当场查。和现在一样的，接受，什么都不记。
    pub(super) fn set_permission(
        &mut self,
        id: CommandId,
        by: By,
        at: Timestamp,
        level: Option<Level>,
        read_only: Option<bool>,
    ) -> Vec<Action> {
        if matches!(level, Some(Level::Other(_))) {
            return vec![rejected(id, Reason::UnknownLevel)];
        }
        let new = Permission {
            level: level.unwrap_or_else(|| self.permission.level.clone()),
            read_only: read_only.unwrap_or(self.permission.read_only),
        };
        if new == self.permission {
            self.recent.insert(id.clone(), Vec::new());
            return vec![accepted(id, Vec::new())];
        }
        let body = Body::PolicyChanged(PolicyChanged {
            policy: None,
            permission: Some(new.clone()),
        });
        let event = self.record(at, by, Some(id.clone()), body);
        self.accept(id, vec![event.seq]);
        let mut events = vec![event];
        let tighter = rank(&new) < rank(&self.effective);
        self.permission = new;
        if let Some(turn) = self.turn.as_mut() {
            turn.refresh = true;
        }
        if tighter {
            self.effective = self.permission.clone();
            events.extend(self.deny_waiting_writes(at));
        }
        if self
            .turn
            .as_ref()
            .is_some_and(|turn| matches!(turn.stage, Stage::Ready))
        {
            events.extend(self.refresh_facts(at));
        }
        vec![Action::Append(events)]
    }

    /// 要请求了：这一轮里切过级别的，放宽的这时生效，环境和权限查一遍，和有效历史里最近
    /// 一块不一样的记成事实（`08-上下文投影.md` C10）。来回切了一圈的，比出来一样，不注入。
    /// 工作目录写这一轮的：派工具带的是它。
    pub(super) fn refresh_facts(&mut self, at: Timestamp) -> Vec<Event> {
        let Some(turn) = self.turn.as_mut() else {
            return Vec::new();
        };
        if !std::mem::take(&mut turn.refresh) {
            return Vec::new();
        }
        let cause = turn.cause.clone();
        let environment = Environment {
            offset: self.environment.offset,
            cwd: turn.cwd.clone(),
        };
        self.effective = self.permission.clone();
        let facts = vec![
            self.policy.facts.env(at, &environment),
            self.policy.facts.permission(&self.permission),
        ];
        changed(&self.history, &By::Kernel, facts)
            .into_iter()
            .map(|fact| self.record(at, By::Kernel, cause.clone(), Body::ContextInjected(fact)))
            .collect()
    }

    /// 实际生效的是不是只读。
    pub(super) fn read_only_now(&self) -> bool {
        rank(&self.effective) == 0
    }
}

/// 一份权限有多宽：只读 0、工作区 1、完全放开 2。不认识的级别按最严的算。
fn rank(permission: &Permission) -> u8 {
    if permission.read_only {
        return 0;
    }
    match permission.level {
        Level::Workspace => 1,
        Level::Full => 2,
        Level::Other(_) => 0,
    }
}
