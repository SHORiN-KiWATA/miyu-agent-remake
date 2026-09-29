//! 看守查清空上下文（`docs/blueprint/compaction.md` 第十四条，施工 6-8 补）：
//!
//! - 有回合在进行的拒绝，`turn_running`；上下文本来就是空的（还算数的最近一次压缩不是一份摘要，它后面也没有人的消息、
//!   回复、工具结果、回报）拒绝，`nothing_to_clear`；拒绝的只有一个回应；
//! - 收下的只追加一批：没有触发的 `turn.started`、替代到它前面那一条的空检查点（`clear`，别的格都没有）、`completed` 的
//!   `turn.ended`，`by` 都是内核、`cause` 都是命令；不请求模型、不跑回合开始的挂接点；
//! - 检查点是还算数的最近一次压缩：之后的请求照它以后的（`watch/compaction.rs`），撤销能撤掉它（`watch/undo.rs`）。

use super::*;
use crate::event::{CompactTrigger, ContextCompacted, TurnEnded, TurnStarted};

/// 送进去之前判出来的：哪个命令，该怎样。
pub(super) enum Expect {
    /// 拒绝，原因码。
    Refused(CommandId, Reason),
    /// 收下：替代到哪。
    Cleared(CommandId, Seq),
}

impl Watch {
    /// 送进一条新的清空之前：照规矩判出该怎样。
    pub(super) fn before_clear(&self, input: &Input) -> Option<Expect> {
        let Input::Command(received) = input else {
            return None;
        };
        if received.command != Command::Clear || !self.fresh(&received.id) {
            return None;
        }
        let id = received.id.clone();
        if self.turn_open() {
            return Some(Expect::Refused(id, Reason::TurnRunning));
        }
        if self.context_is_empty() {
            return Some(Expect::Refused(id, Reason::NothingToClear));
        }
        Some(Expect::Cleared(id, seq(self.last())))
    }

    /// 上下文本来就是空的：还算数的最近一次压缩不是一份摘要（没压过、是清空），它后面也没有人的消息、回复、工具结果、回报。
    fn context_is_empty(&self) -> bool {
        let summarized = self.compactions.live.last().is_some_and(|live| {
            !self.events.iter().any(|event| {
                event.turn == Some(live.turn)
                    && matches!(&event.body, Body::ContextCompacted(compacted)
                        if compacted.upto == live.upto && compacted.trigger == Some(CompactTrigger::Clear))
            })
        });
        !summarized
            && !self.effective_events().iter().any(|event| {
                matches!(
                    event.body,
                    Body::MessageUser(_)
                        | Body::MessageAssistant(_)
                        | Body::ToolResult(_)
                        | Body::JobReported(_)
                        | Body::ChildReported(_)
                )
            })
    }

    /// 送进去以后：拒绝的只有那一个回应；收下的只追加那一批三条，记下它替代到哪，好让写压缩时对上。
    pub(super) fn after_clear(&mut self, actions: &[Action], expect: Option<Expect>) {
        let seed = self.seed;
        let Some(expect) = expect else {
            return;
        };
        let (command, upto) = match expect {
            Expect::Refused(command, reason) => {
                self.seen_paths.insert(match reason {
                    Reason::TurnRunning => "有回合在进行时清空被拒",
                    _ => "上下文为空被拒",
                });
                assert_eq!(
                    actions,
                    [Action::Reply {
                        id: command,
                        outcome: Outcome::Rejected { reason },
                    }],
                    "种子 {seed}：清空该被拒"
                );
                return;
            }
            Expect::Cleared(command, upto) => (command, upto),
        };
        let [Action::Append(events)] = actions else {
            panic!("种子 {seed}：收下的清空只追加一批：{actions:?}");
        };
        let [started, compacted, ended] = events.as_slice() else {
            panic!("种子 {seed}：收下的清空追加三条：{events:?}");
        };
        let turn = TurnId::new(started.seq);
        for event in events {
            assert_eq!(event.turn, Some(turn), "种子 {seed}");
            assert_eq!(event.by, By::Kernel, "种子 {seed}");
            assert_eq!(event.cause.as_ref(), Some(&command), "种子 {seed}");
        }
        assert!(
            matches!(
                &started.body,
                Body::TurnStarted(TurnStarted { trigger: None, .. })
            ),
            "种子 {seed}：清空那一轮没有触发：{started:?}"
        );
        assert_eq!(
            compacted.body,
            Body::ContextCompacted(ContextCompacted {
                upto,
                summary: String::new(),
                trigger: Some(CompactTrigger::Clear),
                instructions: None,
                notes: String::new(),
                restored: Vec::new(),
                refills: None,
            }),
            "种子 {seed}：清空写一个空的检查点，替代到那一轮前面那一条"
        );
        assert_eq!(
            ended.body,
            Body::TurnEnded(TurnEnded {
                reason: EndReason::Completed
            }),
            "种子 {seed}"
        );
        // 不跑回合开始的挂接点：看守当它的挂接点已经跑完了。
        self.done.insert(turn);
        self.compactions.clearing = Some(upto);
    }

    /// 追加了清空的检查点：送进去之前判过要收下的才有，替代到判出来的那一条。它是还算数的最近一次，更早的撤销恢复不了。
    pub(super) fn clear_appended(&mut self, event: &Event, compacted: &ContextCompacted) {
        let seed = self.seed;
        self.seen_paths.insert("清空了");
        assert_eq!(
            self.compactions.clearing.take(),
            Some(compacted.upto),
            "种子 {seed}：没要清空却写了清空的检查点"
        );
        assert_eq!(event.turn, Some(self.open_turn()), "种子 {seed}");
        self.compactions.live.push(super::compaction::Live {
            turn: self.open_turn(),
            upto: compacted.upto,
            blobs: Vec::new(),
        });
        self.undo.cleared();
    }
}
