//! 看守查载入、崩溃、重启（`docs/designs/02-内核.md` 第六节「载入、崩溃、重启」）：
//!
//! - 有计划的重启：正在进行的回合以 `restarted` 结束，排着队的不接着开；
//! - 崩了再载入：日志停在没结束的回合里的，没结果的调用都补上，以 `aborted` 结束，不接着开；
//! - 再起来：最后一轮以 `restarted` 结束、连着被打断的没超过上限，接着开一轮，由排着队的最后一条
//!   触发，没有就由那条结束触发；超过了上限，什么都不补。
//!
//! 只在全都落了盘的时候崩：没落盘的丢了，对内核来说就是在更早那一刻崩的。

use super::*;

/// 看守记着的重启：连着几轮被有计划的重启打断，最后那一轮结束时排着队的消息。
#[derive(Default)]
pub(in super::super) struct Restarts {
    /// 连着几轮以 `restarted` 结束。
    streak: u32,
    /// 最后以 `restarted` 结束的那一轮：那条结束的序号，和结束时排着队的消息。
    last: Option<(Seq, Vec<Seq>)>,
}

impl Watch {
    /// 追加过的事件都落了盘没有。
    pub(in super::super) fn all_stored(&self) -> bool {
        self.events
            .iter()
            .all(|event| self.pushed.contains(&event.seq))
    }

    /// 回合结束：记下是不是被有计划的重启打断的，和那时排着队的消息。
    pub(super) fn note_ended(&mut self, event: &Event, reason: &EndReason) {
        if *reason == EndReason::Restarted {
            self.restarts.streak += 1;
            self.restarts.last = Some((event.seq, self.queued.clone()));
        } else {
            self.restarts.streak = 0;
            self.restarts.last = None;
        }
    }

    /// 回合开始：不是接着干的那一轮（由被打断时排着的最后一条、或者那条结束触发），从头数；被重启
    /// 打断的那一轮不再是最后一轮。
    pub(super) fn note_started(&mut self, trigger: Seq) {
        let resumed = self
            .restarts
            .last
            .as_ref()
            .is_some_and(|(ended, queued)| queued.last().copied().unwrap_or(*ended) == trigger);
        if !resumed {
            self.restarts.streak = 0;
        }
        self.restarts.last = None;
    }

    /// 撤销过：被重启打断的那一轮，再起来也不接了。
    pub(super) fn note_reverted(&mut self) {
        self.restarts.last = None;
    }

    /// 崩一下，或者有计划地重启一下：从落了盘的日志载入一个新会话，查载入吐出来的动作。执行器
    /// 替身在路上的那次请求跟着没了。
    pub(in super::super) fn reload(
        &mut self,
        mut session: Session,
        planned: bool,
        policy: Policy,
    ) -> Session {
        let seed = self.seed;
        if planned {
            let open = self.turn_open();
            self.feed(&mut session, Input::Restarting { at: at(53) });
            if open {
                self.seen_paths.insert("有计划地重启");
                assert!(
                    matches!(self.events.last().map(|event| &event.body),
                        Some(Body::TurnEnded(ended)) if ended.reason == EndReason::Restarted),
                    "种子 {seed}：有计划的重启，那一轮要以 restarted 结束"
                );
            }
            let last = self.last();
            self.feed(&mut session, stored(last));
        }
        let crashed = self.turn_open().then(|| self.open_turn());
        let resume = match (&self.restarts.last, crashed) {
            (Some((ended, queued)), None) if self.restarts.streak <= policy.resumes => {
                Some(queued.last().copied().unwrap_or(*ended))
            }
            _ => None,
        };
        let mut log = vec![self.created()];
        log.extend(self.events.iter().cloned());
        let environment = environment(&self.cwd.clone());
        // 内核照日志里的 `cause` 重建接受过的编号：接受了却什么都没记的，载入以后就忘了。
        self.accepted = log.iter().filter_map(|event| event.cause.clone()).collect();
        let (session, actions) = Session::load(log, at(55), policy, environment)
            .unwrap_or_else(|e| panic!("种子 {seed}：落了盘的日志载入不了：{e}"));
        self.asking = None;
        self.next_block = 0;
        self.open_block = None;
        let appended: Vec<Event> = actions
            .iter()
            .filter_map(|action| match action {
                Action::Append(events) => Some(events.clone()),
                _ => None,
            })
            .flatten()
            .collect();
        match (crashed, resume) {
            (Some(turn), _) => {
                self.seen_paths.insert("崩了以后收尾");
                let unfinished: Vec<CallId> = self
                    .calls_in(turn)
                    .filter(|call| !self.resulted.contains(call))
                    .collect();
                let results: Vec<CallId> = appended
                    .iter()
                    .filter_map(|event| match &event.body {
                        Body::ToolResult(result) => Some(result.call_id),
                        _ => None,
                    })
                    .collect();
                assert_eq!(results, unfinished, "种子 {seed}：没结果的调用都要补上");
                assert!(
                    matches!(appended.last().map(|event| &event.body),
                        Some(Body::TurnEnded(ended)) if ended.reason == EndReason::Aborted),
                    "种子 {seed}：崩了的那一轮以 aborted 结束：{appended:?}"
                );
            }
            (None, Some(trigger)) => {
                self.seen_paths.insert("重启后接着干");
                assert!(
                    matches!(appended.first().map(|event| &event.body),
                        Some(Body::TurnStarted(started)) if started.trigger == trigger),
                    "种子 {seed}：重启以后由 {trigger} 接着开一轮：{appended:?}"
                );
                // 接着干的那一轮，接过去的是被打断时排着的那几句。
                let queued = self.restarts.last.clone().map(|(_, queued)| queued);
                self.undo
                    .picked
                    .insert(TurnId::new(appended[0].seq), queued.unwrap_or_default());
            }
            (None, None) => assert!(
                actions.is_empty(),
                "种子 {seed}：不用收尾、不用接着开的，什么都不补：{actions:?}"
            ),
        }
        for action in actions {
            self.check(action);
        }
        session
    }
}
