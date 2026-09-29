//! 派出去的任务（施工 7-1）：执行器替身在工具结果里报 `job.started`，后台命令和子代理轮着来。账本查编号整份日志不重复、
//! 撤掉的回合里的也算、子代理带会话；随机的撤销、恢复、压缩、崩了载入都照样收得下，载入时整份日志再过一遍账本。
//!
//! 两种回报（`job.reported`、`child.reported`）要执行器的新输入和内核的任务表，随 7-2 接上。

use super::*;
use crate::event::{Effect, JobKind, JobStarted};
use crate::id::{JobId, SessionId};

impl Watch {
    /// 下一个派出去的任务：编号接着日志里用过的最大的往下数，撤掉的回合里的也算，所以不会重复。`n` 为单的派子代理，
    /// 带一个照编号造的会话；为双的派后台命令。
    pub(super) fn some_job(&self, n: usize) -> Effect {
        let last = self
            .events
            .iter()
            .flat_map(|event| match &event.body {
                Body::ToolResult(result) => result.effects.as_slice(),
                _ => &[],
            })
            .filter_map(|effect| match effect {
                Effect::JobStarted(started) => Some(started.job.get()),
                _ => None,
            })
            .max()
            .unwrap_or(0);
        let job = JobId::new(last + 1).expect("从 1 数起");
        let (what, session) = match n % 2 {
            0 => (JobKind::Command, None),
            _ => {
                let id = format!("01a0d78c-ca52-7d19-8b64-{:012x}", job.get());
                (
                    JobKind::Agent,
                    Some(SessionId::parse(&id).expect("照写法造的")),
                )
            }
        };
        Effect::JobStarted(JobStarted {
            job,
            what,
            title: "t".to_string(),
            session,
        })
    }

    /// 记下的工具结果派了任务：走到过这条路。派过任务的回合撤掉过，这一回又派：编号接着撤掉的那几个往下数，账本照收。
    pub(super) fn jobs_seen(&mut self, result: &ToolResult) {
        if !starts_a_job(result) {
            return;
        }
        self.seen_paths.insert("工具结果派了任务");
        let with_jobs: BTreeSet<TurnId> = self
            .events
            .iter()
            .filter(|event| matches!(&event.body, Body::ToolResult(result) if starts_a_job(result)))
            .filter_map(|event| event.turn)
            .collect();
        let undone = self.events.iter().any(|event| {
            matches!(&event.body, Body::TurnReverted(reverted)
                if reverted.turns.iter().any(|turn| with_jobs.contains(turn)))
        });
        if undone {
            self.seen_paths.insert("撤掉派过任务的回合以后又派");
        }
    }
}

/// 这条工具结果派了任务。
fn starts_a_job(result: &ToolResult) -> bool {
    result
        .effects
        .iter()
        .any(|effect| matches!(effect, Effect::JobStarted(_)))
}
