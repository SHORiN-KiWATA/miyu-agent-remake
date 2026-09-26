//! 看守查撤销与恢复（`docs/designs/02-内核.md` 第六节「撤销与恢复」）：
//!
//! - 撤销：看守自己判该接受还是拒绝：有回合在进行的 `turn_running`，不在有效历史里的
//!   `unknown_turn`；接受的只记一条 `turn.reverted`，列的正好是那一轮和它以后还在的每一轮，`by`
//!   是撤销的人；
//! - 恢复：没有能恢复的 `nothing_to_unrevert`；接受的只记一条 `turn.unreverted`，列的正好是最近
//!   一次撤销的那几轮；
//! - 请求照的是撤销、恢复以后的历史：跟着撤的话，看守用自己记的排队算（触发的那句；由上一轮排着的
//!   消息接着开的，上一轮结束时排着的那几句）；
//! - 撤了又恢复、中间没发过请求的，下一次请求接着上一次往下长，不写第一处不同。

use super::*;
use crate::event::{TurnReverted, TurnUnreverted};

/// 看守记着的撤销。
#[derive(Default)]
pub(in super::super) struct Undo {
    /// 还在有效历史里的回合，照先后。
    pub(in super::super) effective: Vec<TurnId>,
    /// 还能恢复的几次撤销：撤了哪几轮、拿走了哪几条，最近的一次在最后。
    stack: Vec<(Vec<TurnId>, BTreeSet<Seq>)>,
    /// 请求里不该有的：撤掉的、撤回的，和撤销、恢复、撤回那几条本身。
    pub(super) gone: BTreeSet<Seq>,
    /// 由上一轮排着的消息接着开的回合，接过去的那几条。
    pub(super) picked: BTreeMap<TurnId, Vec<Seq>>,
    /// 上一次请求以后撤过、恢复过没有；净撤了几次。
    touched: bool,
    net: usize,
    /// 该接着上一次请求往下长的请求，照 `seen`。
    clean: BTreeSet<Seq>,
}

impl Undo {
    /// 有没有能恢复的撤销。
    pub(in super::super) fn can_unrevert(&self) -> bool {
        !self.stack.is_empty()
    }
}

/// 一次新的撤销、恢复，看守判出来该怎样。
pub(super) enum Expect {
    /// 拒绝，这个原因码。
    Refused(Reason),
    /// 记一条撤销，列这几轮。
    Revert(Vec<TurnId>),
    /// 记一条恢复，列这几轮。
    Unrevert(Vec<TurnId>),
}

impl Watch {
    /// 送进一条输入之前：新的撤销、恢复，照规矩判出该怎样。
    pub(super) fn before_undo(&self, input: &Input) -> Option<Expect> {
        let Input::Command(received) = input else {
            return None;
        };
        if !self.fresh(&received.id) {
            return None;
        }
        match &received.command {
            Command::Revert { .. } if self.turn_open() => {
                Some(Expect::Refused(Reason::TurnRunning))
            }
            Command::Revert { turn } => {
                Some(match self.undo.effective.iter().position(|t| t == turn) {
                    Some(k) => Expect::Revert(self.undo.effective[k..].to_vec()),
                    None => Expect::Refused(Reason::UnknownTurn),
                })
            }
            Command::Unrevert => Some(match self.undo.stack.last() {
                Some((turns, _)) => Expect::Unrevert(turns.clone()),
                None => Expect::Refused(Reason::NothingToUnrevert),
            }),
            _ => None,
        }
    }

    /// 送进去以后：照判出来的查。拒绝的只有一个回应；接受的只追加了一条，列的是判出来的那几轮。
    pub(super) fn after_undo(&mut self, actions: &[Action], expect: Option<Expect>) {
        let seed = self.seed;
        let appended: Vec<&Event> = actions
            .iter()
            .filter_map(|action| match action {
                Action::Append(events) => Some(events),
                _ => None,
            })
            .flatten()
            .collect();
        match expect {
            None => {}
            Some(Expect::Refused(reason)) => {
                self.seen_paths.insert(match reason {
                    Reason::NothingToUnrevert => "恢复被拒",
                    _ => "撤销被拒",
                });
                assert!(
                    matches!(actions, [Action::Reply { outcome: Outcome::Rejected { reason: got }, .. }] if *got == reason),
                    "种子 {seed}：应该拒绝，原因码 {}：{actions:?}",
                    reason.code()
                );
            }
            Some(Expect::Revert(turns)) => {
                self.seen_paths.insert("撤销了");
                assert!(
                    matches!(appended.as_slice(), [event] if event.body == Body::TurnReverted(TurnReverted { turns }) && event.by == alice()),
                    "种子 {seed}：撤销只记一条，列的是那一轮和它以后的：{actions:?}"
                );
            }
            Some(Expect::Unrevert(turns)) => {
                self.seen_paths.insert("恢复了");
                assert!(
                    matches!(appended.as_slice(), [event] if event.body == Body::TurnUnreverted(TurnUnreverted { turns }) && event.by == alice()),
                    "种子 {seed}：恢复只记一条，列的是最近一次撤销的那几轮：{actions:?}"
                );
            }
        }
    }

    /// 这一批里的第 `k` 条：记下有效历史里还有哪几轮、能恢复的几次、请求里不该有的几条。
    pub(super) fn undo_check(&mut self, events: &[Event], k: usize) {
        let event = &events[k];
        match &event.body {
            Body::TurnStarted(_) => {
                self.undo.effective.push(TurnId::new(event.seq));
                self.undo.stack.clear();
            }
            Body::MessageWithdrawn(withdrawn) => {
                self.undo.gone.extend(withdrawn.messages.iter().copied());
                self.undo.gone.insert(event.seq);
            }
            Body::TurnReverted(reverted) => {
                let taken = self.undone_by(&reverted.turns);
                self.undo.gone.extend(taken.iter().copied());
                self.undo.gone.insert(event.seq);
                self.undo
                    .effective
                    .retain(|turn| !reverted.turns.contains(turn));
                self.undo.stack.push((reverted.turns.clone(), taken));
                self.undo.touched = true;
                self.undo.net += 1;
                self.note_reverted();
            }
            Body::TurnUnreverted(unreverted) => {
                let (turns, taken) = self.undo.stack.pop().unwrap();
                assert_eq!(turns, unreverted.turns, "种子 {}", self.seed);
                self.undo.gone.retain(|seq| !taken.contains(seq));
                self.undo.gone.insert(event.seq);
                self.undo.effective.extend(turns);
                self.undo.effective.sort();
                self.undo.touched = true;
                self.undo.net -= 1;
            }
            _ => {}
        }
    }

    /// 撤掉这几轮要拿走的：它们的事件；触发它们的、人亲口说的那句；由上一轮排着的消息接着开的，
    /// 接过去的那几句。已经拿走了的不算。
    fn undone_by(&mut self, turns: &[TurnId]) -> BTreeSet<Seq> {
        let gone = &self.undo.gone;
        let mut taken: BTreeSet<Seq> = self
            .events
            .iter()
            .filter(|event| event.turn.is_some_and(|turn| turns.contains(&turn)))
            .map(|event| event.seq)
            .filter(|seq| !gone.contains(seq))
            .collect();
        for turn in turns {
            let trigger = self.events.iter().find_map(|event| match &event.body {
                Body::TurnStarted(started) if event.seq == turn.started() => Some(started.trigger),
                _ => None,
            });
            let said = self.events.iter().find(|event| {
                Some(event.seq) == trigger
                    && matches!(event.body, Body::MessageUser(_))
                    && matches!(event.by, By::Person(_))
            });
            taken.extend(said.map(|event| event.seq));
            let picked = self.undo.picked.get(turn).cloned().unwrap_or_default();
            if picked.len() > 1 {
                self.seen_paths.insert("撤销带走了上一轮排着的");
            }
            taken.extend(picked);
        }
        taken.retain(|seq| !gone.contains(seq));
        taken
    }

    /// 请求模型时：上一次请求以后撤过又都恢复了的，这一次该接着上一次往下长。
    pub(super) fn undo_request(&mut self, seen: Seq) {
        if self.undo.touched && self.undo.net == 0 {
            self.undo.clean.insert(seen);
        }
        self.undo.touched = false;
        self.undo.net = 0;
    }

    /// 记下一次请求：撤了又恢复的，不写第一处不同。
    pub(super) fn undo_called(&mut self, called: &crate::event::ModelCalled) {
        if self.undo.clean.remove(&called.seen) {
            self.seen_paths.insert("恢复以后接着说");
            assert_eq!(
                called.first_difference, None,
                "种子 {}：撤了又恢复的，请求接着上一次往下长",
                self.seed
            );
        }
    }
}
