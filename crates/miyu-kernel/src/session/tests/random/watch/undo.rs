//! 看守查撤销与恢复（`docs/designs/02-内核.md` 第六节「撤销与恢复」）：
//!
//! - 撤销：看守自己判该接受还是拒绝：有回合在进行的 `turn_running`，没有、撤掉了的 `unknown_turn`；
//!   接受的只记一条 `turn.reverted`，列的正好是那一轮和它以后还没撤掉的每一轮，压缩以前的也算，`by`
//!   是撤销的人；
//! - 撤的几轮里有还算数的压缩的（施工 6-9）：先只交出读回日志，从撤完以后还算数的最近一次压缩替代到的下一条起；
//!   读回的对得上才记撤销，对不上的不理；读回的时候来的命令拒绝（`watch/restore.rs`）；
//! - 恢复：没有能恢复的 `nothing_to_unrevert`；接受的只记一条 `turn.unreverted`，列的正好是最近
//!   一次撤销的那几轮，撤掉的压缩跟着回来，不读磁盘；
//! - 检查点换了、重读过文件的，紧跟着出取回原文，要的是新检查点的那几份；
//! - 请求照的是撤销、恢复以后的历史：跟着撤的话，看守用自己记的排队算（触发的那句；由上一轮排着的
//!   消息接着开的，上一轮结束时排着的那几句）；
//! - 撤了又恢复、中间没发过请求的，下一次请求接着上一次往下长，不写第一处不同。

use super::compaction::Live;
use super::*;
use crate::event::{TurnReverted, TurnUnreverted};
use crate::id::ContentHash;

/// 看守记着的撤销。
#[derive(Default)]
pub(in super::super) struct Undo {
    /// 还没撤掉的回合，照先后，压缩以前的也在。
    pub(in super::super) effective: Vec<TurnId>,
    /// 还能恢复的几次撤销：撤了哪几轮、拿走了哪几条、跟着撤掉的压缩，最近的一次在最后。
    stack: Vec<(Vec<TurnId>, BTreeSet<Seq>, Vec<Live>)>,
    /// 撤掉压缩的撤销在读回日志：撤哪几轮、从第几条读起（施工 6-9）。
    pub(in super::super) reading: Option<(Vec<TurnId>, Seq)>,
    /// 请求里不该有的：撤掉的、撤回的，和撤销、恢复、撤回那几条本身。
    pub(super) gone: BTreeSet<Seq>,
    /// 由上一轮排着的消息接着开的回合，接过去的那几条。
    pub(super) picked: BTreeMap<TurnId, Vec<Seq>>,
    /// 上一次请求以后撤过、恢复过没有；净撤了几次。
    touched: bool,
    net: usize,
    /// 上一次请求是摘要请求：下一次和它比，第一处不同就在摘要指令那里，不查接着往下长。这以后清空过的也不查：前缀和压完
    /// 一样重置了（施工 6-8 补）。
    after_summary: bool,
    /// 该接着上一次请求往下长的请求，照 `seen`。
    clean: BTreeSet<Seq>,
}

impl Undo {
    /// 有没有能恢复的撤销。
    pub(in super::super) fn can_unrevert(&self) -> bool {
        !self.stack.is_empty()
    }

    /// 压缩了：更早的撤销恢复不了。压缩以前的回合照样能撤（施工 6-9）。
    pub(super) fn compacted(&mut self) {
        self.stack.clear();
    }

    /// 清空了（施工 6-8 补）：和压缩一样，更早的撤销恢复不了；没有摘要请求，前缀照样重置，下一次请求不和清空以前的比。
    pub(super) fn cleared(&mut self) {
        self.compacted();
        self.after_summary = true;
    }
}

/// 一次新的撤销、恢复、读回，看守判出来该怎样。
pub(super) enum Expect {
    /// 拒绝，这个原因码。
    Refused(Reason),
    /// 撤到还算数的压缩：只交出读回日志，从这一条起（施工 6-9）。
    ReadBack(Vec<TurnId>, Seq),
    /// 过时的、对不上的读回：不理。
    Ignored,
    /// 记一条撤销，列这几轮；是读回来以后记的吗；要取回原文的 blob，空的是不取。
    Revert(Vec<TurnId>, bool, Vec<ContentHash>),
    /// 记一条恢复，列这几轮；放回来的有没有压缩；要取回原文的 blob，空的是不取。
    Unrevert(Vec<TurnId>, bool, Vec<ContentHash>),
}

impl Expect {
    /// 接受的撤销、恢复列的那几轮：看守照它查交没交改回文件（`watch/restore.rs`）。
    pub(super) fn turns(&self) -> Option<Vec<TurnId>> {
        match self {
            Expect::Revert(turns, ..) | Expect::Unrevert(turns, ..) => Some(turns.clone()),
            Expect::Refused(_) | Expect::ReadBack(..) | Expect::Ignored => None,
        }
    }
}

impl Watch {
    /// 送进一条输入之前：新的撤销、恢复，读回的日志，照规矩判出该怎样。不写回合编号的撤最后一轮（施工 4-7 下）。
    pub(super) fn before_undo(&mut self, input: &Input) -> Option<Expect> {
        if let Input::ReadBack { from, events, .. } = input {
            // 重做在等的读回，照重做判（`watch/redo.rs`，施工 4-7 再补）。
            if self.redo_reading() {
                return None;
            }
            return Some(match self.undo.reading.clone() {
                Some((turns, reading)) if reading == *from && *events == self.log_from(reading) => {
                    let recall = self.recall_after(turns[0]);
                    Expect::Revert(turns, true, recall)
                }
                _ => Expect::Ignored,
            });
        }
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
            Command::Revert { turn: None } => Some(match self.undo.effective.last() {
                Some(last) => {
                    self.seen_paths.insert("撤最后一轮");
                    self.reverting(vec![*last])
                }
                None => Expect::Refused(Reason::NothingToRevert),
            }),
            Command::Revert { turn: Some(turn) } => {
                Some(match self.undo.effective.iter().position(|t| t == turn) {
                    Some(k) => self.reverting(self.undo.effective[k..].to_vec()),
                    None => Expect::Refused(Reason::UnknownTurn),
                })
            }
            Command::Unrevert => Some(match self.undo.stack.last() {
                Some((turns, _, back)) => Expect::Unrevert(
                    turns.clone(),
                    !back.is_empty(),
                    back.last()
                        .map(|live| live.blobs.clone())
                        .unwrap_or_default(),
                ),
                None => Expect::Refused(Reason::NothingToUnrevert),
            }),
            _ => None,
        }
    }

    /// 从第一轮起撤这几轮：撤到还算数的压缩的先读回，从撤完以后还算数的最近一次压缩替代到的下一条起；别的当场记。
    pub(super) fn reverting(&self, turns: Vec<TurnId>) -> Expect {
        let first = turns[0];
        let live = &self.compactions.live;
        if !live.iter().any(|live| live.turn >= first) {
            return Expect::Revert(turns, false, Vec::new());
        }
        let from = live
            .iter()
            .rev()
            .find(|live| live.turn < first)
            .map_or(Seq::FIRST, |live| live.upto.next());
        Expect::ReadBack(turns, from)
    }

    /// 还算数的压缩各在哪一轮：随机的撤销偶尔撤到它们（施工 6-9）。
    pub(in super::super) fn compaction_turns(&self) -> Vec<TurnId> {
        self.compactions.live.iter().map(|live| live.turn).collect()
    }

    /// 从 `first` 起撤完以后的检查点重读过的文件：要取回原文的 blob。
    pub(super) fn recall_after(&self, first: TurnId) -> Vec<ContentHash> {
        self.compactions
            .live
            .iter()
            .rev()
            .find(|live| live.turn < first)
            .map(|live| live.blobs.clone())
            .unwrap_or_default()
    }

    /// 日志里第 `from` 条起的事件，造会话那一条算在里面：读回日志照它回（施工 6-9）。
    pub(in super::super) fn log_from(&self, from: Seq) -> Vec<Event> {
        std::iter::once(self.created())
            .chain(self.events.iter().cloned())
            .filter(|event| event.seq >= from)
            .collect()
    }

    /// 送进去以后：照判出来的查。拒绝的只有一个回应；接受的只追加了一条，列的是判出来的那几轮；检查点换了、重读过
    /// 文件的，紧跟着取回原文。读回日志时到的转述（施工 8-17）先放着、读完才记，排在这一批后面，不算撤销的。
    pub(super) fn after_undo(&mut self, actions: &[Action], expect: Option<Expect>) {
        let seed = self.seed;
        let appended: Vec<&Event> = actions
            .iter()
            .filter_map(|action| match action {
                Action::Append(events) => Some(events),
                _ => None,
            })
            .flatten()
            .filter(|event| !matches!(event.body, Body::ImageDescribed(_)))
            .collect();
        let recalls: Vec<&Vec<ContentHash>> = actions
            .iter()
            .filter_map(|action| match action {
                Action::Recall { blobs } => Some(blobs),
                _ => None,
            })
            .collect();
        let recalled = |blobs: &Vec<ContentHash>| match blobs.is_empty() {
            true => recalls.is_empty(),
            false => recalls == [blobs] && matches!(actions.get(1), Some(Action::Recall { .. })),
        };
        match expect {
            None => {}
            Some(Expect::Refused(reason)) => {
                self.seen_paths.insert(match reason {
                    Reason::NothingToUnrevert => "恢复被拒",
                    Reason::NothingToRevert => "没有能撤的被拒",
                    _ => "撤销被拒",
                });
                assert!(
                    matches!(actions, [Action::Reply { outcome: Outcome::Rejected { reason: got, .. }, .. }] if *got == reason),
                    "种子 {seed}：应该拒绝，原因码 {}：{actions:?}",
                    reason.code()
                );
            }
            Some(Expect::ReadBack(turns, from)) => {
                self.seen_paths.insert("撤到压缩先读回日志");
                assert!(
                    matches!(actions, [Action::ReadBack { from: got }] if *got == from),
                    "种子 {seed}：撤到还算数的压缩，先只交出读回日志，从 {from} 起：{actions:?}"
                );
                self.undo.reading = Some((turns, from));
            }
            Some(Expect::Ignored) => {
                self.seen_paths.insert("读回的对不上不理");
                assert!(actions.is_empty(), "种子 {seed}：对不上的读回：{actions:?}");
            }
            Some(Expect::Revert(turns, read_back, recall)) => {
                self.seen_paths.insert("撤销了");
                if read_back {
                    self.seen_paths.insert("撤掉了压缩");
                    self.undo.reading = None;
                }
                assert!(
                    matches!(appended.as_slice(), [event] if event.body == Body::TurnReverted(TurnReverted { turns }) && event.by == alice()),
                    "种子 {seed}：撤销只记一条，列的是那一轮和它以后的：{actions:?}"
                );
                assert!(
                    recalled(&recall),
                    "种子 {seed}：取回原文要 {recall:?}：{actions:?}"
                );
            }
            Some(Expect::Unrevert(turns, back, recall)) => {
                self.seen_paths.insert("恢复了");
                if back {
                    self.seen_paths.insert("恢复了压缩");
                }
                // 后面还有的，是记在一边的回报接着开的那一轮（施工 7-2，`watch/reports.rs` 查）。
                assert!(
                    matches!(appended.as_slice(), [event, ..] if event.body == Body::TurnUnreverted(TurnUnreverted { turns }) && event.by == alice()),
                    "种子 {seed}：恢复只记一条，列的是最近一次撤销的那几轮：{actions:?}"
                );
                assert!(
                    recalled(&recall),
                    "种子 {seed}：取回原文要 {recall:?}：{actions:?}"
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
                let first = reverted.turns[0];
                let kept = self
                    .compactions
                    .live
                    .partition_point(|live| live.turn < first);
                let compactions = self.compactions.live.split_off(kept);
                self.undo
                    .stack
                    .push((reverted.turns.clone(), taken, compactions));
                self.undo.touched = true;
                self.undo.net += 1;
                self.note_reverted();
            }
            Body::TurnUnreverted(unreverted) => {
                let (turns, taken, compactions) = self.undo.stack.pop().unwrap();
                assert_eq!(turns, unreverted.turns, "种子 {}", self.seed);
                self.compactions.live.extend(compactions);
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
    /// 接过去的那几句；触发的那句没有回合编号的（空闲时说的、重做重发的），和它同一个命令、也没有回合编号的人的话
    /// （施工 4-7 再补）。已经拿走了的不算。
    pub(super) fn undone_by(&mut self, turns: &[TurnId]) -> BTreeSet<Seq> {
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
                Body::TurnStarted(started) if event.seq == turn.started() => started.trigger,
                _ => None,
            });
            let said = self.events.iter().find(|event| {
                Some(event.seq) == trigger
                    && matches!(event.body, Body::MessageUser(_))
                    && matches!(event.by, By::Person(_))
            });
            taken.extend(said.map(|event| event.seq));
            let resent: Vec<Seq> = said
                .filter(|said| said.turn.is_none() && said.cause.is_some())
                .map(|said| {
                    self.events
                        .iter()
                        .filter(|event| {
                            event.turn.is_none()
                                && event.cause == said.cause
                                && matches!(event.body, Body::MessageUser(_))
                                && matches!(event.by, By::Person(_))
                        })
                        .map(|event| event.seq)
                        .collect()
                })
                .unwrap_or_default();
            if resent.len() > 1 {
                self.seen_paths.insert("撤销带走了重做重发的几句");
            }
            taken.extend(resent);
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
        let after_summary = std::mem::take(&mut self.undo.after_summary);
        if self.undo.touched && self.undo.net == 0 && !after_summary {
            self.undo.clean.insert(seen);
        }
        self.undo.touched = false;
        self.undo.net = 0;
    }

    /// 发了摘要请求：它截到 N，不一定接着上一次往下长（上一次出错、没回复的，N 在那以前），不查；撤过、恢复过的账
    /// 照样清掉，压完的主请求前缀本来就重置。
    pub(super) fn undo_summary(&mut self) {
        self.undo.touched = false;
        self.undo.net = 0;
        self.undo.after_summary = true;
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
