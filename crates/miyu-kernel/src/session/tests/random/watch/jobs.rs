//! 派出去的任务（施工 7-1）：执行器替身在工具结果里报 `job.started`，后台命令和子代理轮着来。账本查编号整份日志不重复、
//! 撤掉的回合里的也算、子代理带会话；随机的撤销、恢复、压缩、崩了载入都照样收得下，载入时整份日志再过一遍账本。
//!
//! 两种回报（`job.reported`、`child.reported`）施工 7-2 接上，在 `watch/reports.rs`。

use super::*;
use crate::event::{Effect, JobKind, JobStarted};
use crate::id::{JobId, SessionId};

impl Watch {
    /// 下一个派出去的任务：编号接着日志里用过的最大的往下数，撤掉的回合里的也算，所以不会重复。`n` 为单的派子代理，
    /// 带一个照编号造的会话；为双的派后台命令。每隔两个，编号接在前缀 `j9` 后面（施工 7-1 补）：带前缀的和不带的混在一份
    /// 日志里，像旧日志里子会话派的 `j1` 后面接着新派的 `j2.2`，照最后一段往下数也不重。
    pub(super) fn some_job(&self, n: usize) -> Effect {
        let last = self
            .events
            .iter()
            .flat_map(|event| match &event.body {
                Body::ToolResult(result) => result.effects.as_slice(),
                _ => &[],
            })
            .filter_map(|effect| match effect {
                Effect::JobStarted(started) => Some(started.job.last()),
                _ => None,
            })
            .max()
            .unwrap_or(0);
        let job = match (n / 2) % 2 {
            0 => JobId::new(last + 1),
            _ => JobId::new(9).and_then(|prefix| prefix.under(last + 1)),
        }
        .expect("从 1 数起");
        let (what, session) = match n % 2 {
            0 => (JobKind::Command, None),
            _ => {
                let id = format!("01a0d78c-ca52-7d19-8b64-{:012x}", job.last());
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
