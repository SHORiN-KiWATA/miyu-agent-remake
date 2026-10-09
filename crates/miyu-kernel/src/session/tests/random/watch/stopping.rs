//! 看守查打断和停着的（`docs/designs/02-内核.md` 第六节「打断和急着插话」，蓝图 `kernel/session.md`「打断」，
//! 施工 4-9 再补一）：
//!
//! - 一次新的打断：回合开着的，这一批以被打断的 `turn.ended` 收尾；没开着的，拒绝，原因码 `not_running`；
//! - 有在跑的改文件的调用的，叫它停，同一批交出一次到点叫醒，这一批不结束回合；
//! - 停着的交回来：停在改之前的补「已取消」，`by` 是打断的人；改完了的照工具交的记；都交回来了才结束；
//! - 到点了、又打断了一次：这一批结束回合，还没交回来的补了「已取消」才叫执行器停下。

use super::*;

/// 停着的。
#[derive(Debug, Default)]
pub(in super::super) struct Stopping {
    /// 叫它停过、还没结果的调用。
    pub(in super::super) calls: BTreeSet<CallId>,
    /// 在等停着的：那次打断，排着队的怎么办。
    pub(in super::super) queued: Option<Queued>,
    /// 等停着的到点叫醒的记号。
    pub(in super::super) wake: Option<Seq>,
}

/// 送进去的这一条，对停着的是什么。
pub(super) enum StopInput {
    /// 停着的 `call_id` 交回来了：停在了改之前没有，是不是最后一个。
    Done {
        call_id: CallId,
        stopped: bool,
        last: bool,
    },
    /// 等停着的到点了。
    Woke,
}

impl Watch {
    /// 一次新的打断吐出来的动作：`was_open` 是送进去之前回合开着没有，`queued` 是排着队的怎么办。
    pub(super) fn interrupted(&mut self, actions: &[Action], was_open: bool, queued: Queued) {
        let seed = self.seed;
        if !was_open {
            self.seen_paths.insert("空闲时打断被拒");
            assert!(
                matches!(
                    actions,
                    [Action::Reply {
                        outcome: Outcome::Rejected {
                            reason: Reason::NotRunning,
                            ..
                        },
                        ..
                    }]
                ),
                "种子 {seed}：空闲时的打断应该拒绝：{actions:?}"
            );
            return;
        }
        self.seen_paths.insert("打断了回合");
        let again = self.stopping.queued.is_some();
        let stops = actions
            .iter()
            .any(|action| matches!(action, Action::StopTool { .. }));
        let ended = ends_interrupted(actions);
        if stops {
            self.seen_paths.insert("打断时等停着的");
            assert!(!again, "种子 {seed}：已经在等停着的，又叫停");
            assert!(
                !ended,
                "种子 {seed}：有停着的，这一批不该结束回合：{actions:?}"
            );
            self.stopping.queued = Some(queued);
            return;
        }
        if again {
            self.seen_paths.insert("又打断就不等了");
        }
        assert!(
            ended,
            "种子 {seed}：打断以后回合没以被打断结束：{actions:?}"
        );
    }

    /// 有在跑的改文件的调用，还没叫它停。
    pub(in super::super) fn write_running(&self) -> bool {
        self.stopping.calls.is_empty()
            && self
                .running
                .iter()
                .any(|call_id| self.name_of(*call_id) == "write")
    }

    /// 叫它停：一次新的打断里，在跑的改文件的调用。
    pub(super) fn stop_asked(&mut self, call_id: CallId) {
        let seed = self.seed;
        self.seen_paths.insert("叫它停");
        assert!(
            self.interrupting.is_some(),
            "种子 {seed}：不是打断，却叫 {call_id} 停"
        );
        assert!(
            self.running.contains(&call_id),
            "种子 {seed}：{call_id} 不在跑，却叫它停"
        );
        assert_eq!(
            self.name_of(call_id),
            "write",
            "种子 {seed}：叫它停的 {call_id} 不是改文件的调用"
        );
        assert!(
            self.stopping.calls.insert(call_id),
            "种子 {seed}：{call_id} 叫停了两次"
        );
    }

    /// 交出到点叫醒：刚叫了停、还没交出过的，是等停着的；别的是重试（`watch/model.rs`）。
    pub(super) fn wake_asked(&mut self, seen: Seq) {
        if self.stopping.wake.is_none() && !self.stopping.calls.is_empty() {
            self.seen_paths.insert("等停着的到点叫醒");
            self.stopping.wake = Some(seen);
        } else {
            self.retry_wake(seen);
        }
    }

    /// 送进一条输入之前：是不是停着的交回来了、等停着的到点了。
    pub(super) fn before_stop(&self, input: &Input) -> Option<StopInput> {
        match input {
            Input::ToolDone {
                call_id, stopped, ..
            } if self.stopping.calls.contains(call_id) => Some(StopInput::Done {
                call_id: *call_id,
                stopped: *stopped,
                last: self.stopping.calls.len() == 1,
            }),
            Input::Woke { seen, .. }
                if self.stopping.wake == Some(*seen) && !self.stopping.calls.is_empty() =>
            {
                Some(StopInput::Woke)
            }
            _ => None,
        }
    }

    /// 送进去以后：停着的交回来了，照规矩记；都交回来了、到点了，这一批结束回合。
    pub(super) fn after_stop(&mut self, actions: &[Action], input: Option<StopInput>) {
        let seed = self.seed;
        let Some(input) = input else {
            return;
        };
        let ended = ends_interrupted(actions);
        match input {
            StopInput::Done {
                call_id,
                stopped,
                last,
            } => {
                let (event, result) = actions
                    .iter()
                    .filter_map(|action| match action {
                        Action::Append(events) => Some(events),
                        _ => None,
                    })
                    .flatten()
                    .find_map(|event| match &event.body {
                        Body::ToolResult(result) if result.call_id == call_id => {
                            Some((event, result))
                        }
                        _ => None,
                    })
                    .unwrap_or_else(|| panic!("种子 {seed}：停着的 {call_id} 交回来了，没记结果"));
                if stopped {
                    self.seen_paths.insert("停着的停在改之前");
                    assert_eq!(result.status, ToolStatus::Cancelled, "种子 {seed}");
                    assert!(
                        matches!(event.by, By::Person(_)),
                        "种子 {seed}：停在改之前的，补的结果是打断的人的"
                    );
                    assert!(
                        result.effects.is_empty(),
                        "种子 {seed}：停在改之前的没有效果"
                    );
                } else {
                    self.seen_paths.insert("停着的改完了");
                    assert_eq!(event.by, By::Tool(crate::origin::Tool { call_id }));
                    assert_ne!(result.status, ToolStatus::Cancelled, "种子 {seed}");
                }
                assert_eq!(
                    ended, last,
                    "种子 {seed}：停着的都交回来了才结束回合：{actions:?}"
                );
                if last {
                    self.seen_paths.insert("停着的都交回来了收尾");
                }
            }
            StopInput::Woke => {
                self.seen_paths.insert("到点不等了");
                assert!(
                    ended,
                    "种子 {seed}：等停着的到点了，这一批要结束回合：{actions:?}"
                );
            }
        }
    }

    /// 回合结束：在等停着的，只能是被打断、重启、崩了结束，停着的都有了结果。
    pub(super) fn stop_ended(&mut self, reason: &EndReason) {
        let seed = self.seed;
        if self.stopping.queued.is_some() {
            assert!(
                matches!(
                    reason,
                    EndReason::Interrupted | EndReason::Restarted | EndReason::Aborted
                ),
                "种子 {seed}：在等停着的，回合却以 {reason:?} 结束"
            );
            assert!(
                self.stopping.calls.is_empty(),
                "种子 {seed}：还有停着的没结果，回合就结束了"
            );
        }
        self.stopping = Stopping::default();
    }
}

/// 这一批里有没有被打断的 `turn.ended`。
fn ends_interrupted(actions: &[Action]) -> bool {
    actions.iter().any(|action| {
        match action {
        Action::Append(events) => events.iter().any(|event| {
            matches!(&event.body, Body::TurnEnded(ended) if ended.reason == EndReason::Interrupted)
        }),
        _ => false,
    }
    })
}
