//! 看守查手动压缩（`docs/blueprint/compaction.md` 第七条，施工 6-8）：
//!
//! - 有回合在进行的拒绝，`turn_running`；照规矩算不出 N 的（没交过限额、前面没有能压的）拒绝，`nothing_to_compact`；
//!   拒绝的只有一个回应；
//! - 收下的只追加一条没有触发的 `turn.started`，`by` 是内核、`cause` 是命令；这一轮不跑回合开始的挂接点、不注入事实、
//!   只发摘要请求（`watch/compaction.rs` 照收命令时算的 N 和要求查它）；
//! - 取到了摘要写压缩（`manual`，带着要求），同一批结束；失败不数进熔断（`watch/breaker.rs`）；被重启打断的不接着干
//!   （`watch/load.rs`）。
//!
//! 回合开始的挂接点怎么叫也在这里查（`watch.rs` 放不下）：开头都推送了，一轮只叫一次。

use super::*;

/// 手动压缩单开的那一轮：哪一轮，收命令时看守算的 N，人附的要求（只有空白的当没附）。
#[derive(Clone)]
pub(in super::super) struct ManualTurn {
    pub(super) turn: TurnId,
    pub(super) upto: Seq,
    pub(super) instructions: Option<String>,
}

/// 送进去之前判出来的：哪个命令，该怎样。
pub(super) enum Expect {
    /// 拒绝，原因码。
    Refused(CommandId, Reason),
    /// 收下，单开一轮：替代到哪、要求。
    Opened(CommandId, Seq, Option<String>),
}

impl Watch {
    /// 叫跑回合开始的挂接点：这一轮的开头都推送了，一轮只叫一次；手动压缩那一轮不叫（施工 6-8）。
    pub(super) fn start_hooks(&mut self, turn: TurnId) {
        let seed = self.seed;
        assert!(
            self.compactions
                .manual
                .as_ref()
                .is_none_or(|manual| manual.turn != turn),
            "种子 {seed}：手动压缩那一轮跑了回合开始的挂接点"
        );
        assert!(
            self.opening(turn).all(|event| self.pushed.contains(&event)),
            "种子 {seed}：回合 {turn} 的开头还没落盘就跑挂接点"
        );
        assert!(
            self.hooked.insert(turn),
            "种子 {seed}：回合 {turn} 叫了两次"
        );
    }

    /// 请求模型：手动压缩那一轮只发摘要请求，不查事实；别的回合照常查（`watch/permission.rs`）。
    pub(super) fn manual_request(&mut self, request: &Request) {
        match self.manual_turn() {
            Some(_) => assert!(
                Watch::is_summary(request),
                "种子 {}：手动压缩那一轮发了主请求",
                self.seed
            ),
            None => self.permission_request(),
        }
    }

    /// 推给头的都是开着的那一轮的；压好了照写下压缩时的回合对（`watch/compaction.rs`）：手动压缩的那一轮同一批就结束了。
    pub(super) fn transient_turn(&self, transient: &Transient) {
        if !matches!(transient.body, TransientBody::CompactionDone(_)) {
            assert_eq!(transient.turn, Some(self.open_turn()), "种子 {}", self.seed);
        }
    }

    /// 开着的回合是不是手动压缩单开的那一轮。
    pub(in super::super) fn manual_turn(&self) -> Option<&ManualTurn> {
        let manual = self.compactions.manual.as_ref()?;
        (self.turn_open() && manual.turn == self.open_turn()).then_some(manual)
    }

    /// 送进一条新的手动压缩之前：照规矩判出该怎样。
    pub(super) fn before_compact(&mut self, input: &Input) -> Option<Expect> {
        let Input::Command(received) = input else {
            return None;
        };
        let Command::Compact { instructions } = &received.command else {
            return None;
        };
        if !self.fresh(&received.id) {
            return None;
        }
        let id = received.id.clone();
        if self.turn_open() {
            return Some(Expect::Refused(id, Reason::TurnRunning));
        }
        let Some(upto) = self.expected_manual_upto() else {
            return Some(Expect::Refused(id, Reason::NothingToCompact));
        };
        let instructions = instructions.clone().filter(|text| !text.trim().is_empty());
        Some(Expect::Opened(id, upto, instructions))
    }

    /// 手动压缩替代到哪：照还没开的那一轮算，没有触发，至多到落了盘的最后一条；尾巴的预算是 min(30, 压缩线的四分之一)，
    /// 没有压缩线的是 30。没交过限额的算不了，没有。
    fn expected_manual_upto(&mut self) -> Option<Seq> {
        self.compactions.limits.as_ref()?;
        let budget = self.line().map_or(30, |line| (line / 4).min(30));
        let stored = self.pushed.iter().max().copied()?;
        self.expected_cut(seq(self.last() + 1), None, Some(budget), false, stored)
    }

    /// 送进去以后：拒绝的只有那一个回应；收下的只追加了一条没有触发的回合开始，记下这一轮。
    pub(super) fn after_compact(&mut self, actions: &[Action], expect: Option<Expect>) {
        let seed = self.seed;
        let Some(expect) = expect else {
            return;
        };
        match expect {
            Expect::Refused(command, reason) => {
                self.seen_paths.insert(match reason {
                    Reason::TurnRunning => "有回合在进行时手动压缩被拒",
                    _ => "没有能压的被拒",
                });
                assert_eq!(
                    actions,
                    [Action::Reply {
                        id: command.clone(),
                        outcome: Outcome::Rejected { reason },
                    }],
                    "种子 {seed}：手动压缩该被拒"
                );
            }
            Expect::Opened(command, upto, instructions) => {
                let [Action::Append(events)] = actions else {
                    panic!("种子 {seed}：收下的手动压缩只追加一批：{actions:?}");
                };
                let [started] = events.as_slice() else {
                    panic!("种子 {seed}：收下的手动压缩只追加回合开始：{events:?}");
                };
                assert!(
                    matches!(&started.body, Body::TurnStarted(opened) if opened.trigger.is_none()),
                    "种子 {seed}：手动压缩那一轮没有触发：{started:?}"
                );
                assert_eq!(started.by, By::Kernel);
                assert_eq!(started.cause, Some(command));
                let turn = TurnId::new(started.seq);
                // 不跑回合开始的挂接点：看守当它的挂接点已经跑完了，好让请求时的查照常。
                self.done.insert(turn);
                self.compactions.manual = Some(ManualTurn {
                    turn,
                    upto,
                    instructions,
                });
            }
        }
    }
}
