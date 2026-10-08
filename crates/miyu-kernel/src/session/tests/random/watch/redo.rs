//! 看守查重做（`docs/blueprint/kernel/history.md`「重做」，施工 4-7 再补）：
//!
//! - 有回合在进行的，`turn_running`；一轮都没有、最后一轮不是人说的话开的，`not_redoable`；换过的那一句一块都不剩的，
//!   `empty_message`；
//! - 最后一轮里有还算数的压缩的，先只交出读回日志；它的开头、触发的那一条压掉了的，读回来才判，不能的照样拒绝；
//! - 收下的先记一条撤最后一轮的 `turn.reverted`；接着（改过文件的，改完了、结局后面）重发那一轮撤掉的人的话：除了那一轮
//!   中途来的，照先后，内容照原来的（开这一轮的那一句照交来的换字、换附件），`by` 照原来的，`cause` 是这个命令，
//!   不带回合编号；由开这一轮的那一句的新的那一条开一轮。撤掉的、重发的算不算数由 `watch/undo.rs` 照样记。

use super::*;
use crate::block::{Block, Text};
use crate::event::{MessageUser, TurnReverted};
use crate::id::ContentHash;

/// 开这一轮的那一句换什么：字、附件，`None` 的照原来的。
type Change = (Option<Vec<Block>>, Option<Vec<Block>>);

/// 看守记着的重做。
#[derive(Default)]
pub(in super::super) struct Redoing {
    /// 等读回日志的那一次：命令、换什么。
    reading: Option<(CommandId, Change)>,
    /// 撤销记下了、等改回文件的结局才重发的那一次。
    restoring: Option<Resend>,
}

/// 该怎么重发：哪个命令、原来那几句（序号，照先后）、开这一轮的那一句、换什么。
#[derive(Clone)]
pub(super) struct Resend {
    command: CommandId,
    said: Vec<Seq>,
    opener: Option<Seq>,
    change: Change,
}

/// 一次新的重做、它的读回，看守判出来该怎样。
pub(super) enum Expect {
    /// 拒绝，这个原因码；读回来以后才拒的，读回跟着了结。
    Refused(Reason, bool),
    /// 只交出读回日志：撤这一轮，从这一条起。
    ReadBack(CommandId, TurnId, Seq, Change),
    /// 过时的、对不上的读回：不理。
    Ignored,
    /// 记撤销、重发：撤这一轮；是读回来以后记的吗；要取回原文的 blob。
    Redone(TurnId, bool, Vec<ContentHash>, CommandId, Change),
}

impl Expect {
    /// 接受了、当场记撤销的那一轮：看守照它查交没交改回文件（`watch/restore.rs`）。
    pub(super) fn turns(&self) -> Option<Vec<TurnId>> {
        match self {
            Expect::Redone(turn, ..) => Some(vec![*turn]),
            _ => None,
        }
    }
}

impl Watch {
    /// 在等重做的读回：撤销那边的看守不照撤销判这一次读回。
    pub(super) fn redo_reading(&self) -> bool {
        self.redoing.reading.is_some()
    }

    /// 送进一条输入之前：新的重做、重做在等的读回，照规矩判出该怎样。
    pub(super) fn before_redo(&mut self, input: &Input) -> Option<Expect> {
        if let Input::ReadBack { from, events, .. } = input {
            let (command, change) = self.redoing.reading.clone()?;
            return Some(match self.undo.reading.clone() {
                Some((turns, reading)) if reading == *from && *events == self.log_from(reading) => {
                    let turn = turns[0];
                    match (self.person_opened(turn), self.emptied(turn, &change)) {
                        (false, _) => Expect::Refused(Reason::NotRedoable, true),
                        (true, true) => Expect::Refused(Reason::EmptyMessage, true),
                        (true, false) => {
                            let recall = self.recall_after(turn);
                            Expect::Redone(turn, true, recall, command, change)
                        }
                    }
                }
                _ => Expect::Ignored,
            });
        }
        let Input::Command(received) = input else {
            return None;
        };
        let Command::Redo { text, attachments } = &received.command else {
            return None;
        };
        if !self.fresh(&received.id) {
            return None;
        }
        let change = (text.clone(), attachments.clone());
        if self.turn_open() {
            return Some(Expect::Refused(Reason::TurnRunning, false));
        }
        let Some(turn) = self.undo.effective.last().copied() else {
            return Some(Expect::Refused(Reason::NotRedoable, false));
        };
        let from = match self.reverting(vec![turn]) {
            super::undo::Expect::ReadBack(_, from) => Some(from),
            _ => None,
        };
        let command = received.id.clone();
        let emptied = self.emptied(turn, &change);
        Some(match (self.known_opened(turn), from) {
            (Some(false), _) | (None, None) => Expect::Refused(Reason::NotRedoable, false),
            (Some(true), _) if emptied => Expect::Refused(Reason::EmptyMessage, false),
            (_, Some(from)) => Expect::ReadBack(command, turn, from, change),
            (Some(true), None) => Expect::Redone(turn, false, Vec::new(), command, change),
        })
    }

    /// 回合 `turn` 是人说的话开的：照整份日志看它的触发。
    fn person_opened(&self, turn: TurnId) -> bool {
        self.trigger_of(turn)
            .and_then(|trigger| self.find(trigger))
            .is_some_and(|event| {
                matches!(event.body, Body::MessageUser(_)) && matches!(event.by, By::Person(_))
            })
    }

    /// 内核照内存里的有效历史判得出吗：开头、触发的那一条都在还算数的最近一次压缩替代到的以后，判得出，交回判的；
    /// 有一样压掉了，判不出，没有。
    fn known_opened(&self, turn: TurnId) -> Option<bool> {
        let upto = self.compactions.upto();
        let kept = |seq: Seq| upto.is_none_or(|upto| seq > upto);
        if !kept(turn.started()) {
            return None;
        }
        match self.trigger_of(turn) {
            None => Some(false),
            Some(trigger) if !kept(trigger) => None,
            Some(_) => Some(self.person_opened(turn)),
        }
    }

    /// 回合 `turn` 是人开的、开它的那一句照 `change` 换过以后一块都不剩。
    fn emptied(&self, turn: TurnId, change: &Change) -> bool {
        self.trigger_of(turn)
            .and_then(|trigger| self.find(trigger))
            .is_some_and(|event| match &event.body {
                Body::MessageUser(MessageUser {
                    blocks,
                    venue: None,
                }) => edited(blocks, change).is_empty(),
                _ => false,
            })
    }

    /// 回合 `turn` 的触发。
    fn trigger_of(&self, turn: TurnId) -> Option<Seq> {
        self.find(turn.started())
            .and_then(|event| match &event.body {
                Body::TurnStarted(started) => started.trigger,
                _ => None,
            })
    }

    /// 日志里第 `seq` 条。
    fn find(&self, seq: Seq) -> Option<&Event> {
        self.events.iter().find(|event| event.seq == seq)
    }

    /// 送进去以后：照判出来的查。收下的记撤销、重发；等改回文件的，结局回来再查重发（[`Watch::redo_restored`]）。
    pub(super) fn after_redo(&mut self, actions: &[Action], expect: Option<Expect>) {
        let seed = self.seed;
        match expect {
            None => {}
            Some(Expect::Refused(reason, read_back)) => {
                self.seen_paths.insert(match (reason, read_back) {
                    (Reason::NotRedoable, true) => "读回来才判出不能重做",
                    (Reason::NotRedoable, false) => "不能重做被拒",
                    _ => "重做被拒",
                });
                if read_back {
                    self.undo.reading = None;
                    self.redoing.reading = None;
                }
                // 读回来才拒的，读回的时候到的后台命令结束跟在后面照常记（`revert.rs`）。
                assert!(
                    matches!(actions.first(), Some(Action::Reply { outcome: Outcome::Rejected { reason: got }, .. }) if *got == reason),
                    "种子 {seed}：重做应该拒绝，原因码 {}：{actions:?}",
                    reason.code()
                );
                assert!(
                    read_back || actions.len() == 1,
                    "种子 {seed}：当场拒的只有一个回应：{actions:?}"
                );
            }
            Some(Expect::ReadBack(command, turn, from, change)) => {
                self.seen_paths.insert("重做先读回日志");
                assert!(
                    matches!(actions, [Action::ReadBack { from: got }] if *got == from),
                    "种子 {seed}：重做撤到还算数的压缩，先只交出读回日志，从 {from} 起：{actions:?}"
                );
                self.undo.reading = Some((vec![turn], from));
                self.redoing.reading = Some((command, change));
            }
            Some(Expect::Ignored) => {
                assert!(actions.is_empty(), "种子 {seed}：对不上的读回：{actions:?}");
            }
            Some(Expect::Redone(turn, read_back, recall, command, change)) => {
                self.redone(
                    actions,
                    turn,
                    read_back,
                    &recall,
                    Resend {
                        command,
                        said: Vec::new(),
                        opener: self.trigger_of(turn),
                        change,
                    },
                );
            }
        }
    }

    /// 收下的重做：只撤这一轮；那一轮改过文件的，先只记撤销、交出改回文件，等结局；没改过的，同一批重发。
    fn redone(
        &mut self,
        actions: &[Action],
        turn: TurnId,
        read_back: bool,
        recall: &[ContentHash],
        mut resend: Resend,
    ) {
        let seed = self.seed;
        self.seen_paths.insert("重做了");
        if read_back {
            self.seen_paths.insert("重做撤掉了压缩");
            self.undo.reading = None;
            self.redoing.reading = None;
        }
        if resend.change.0.is_some() {
            self.seen_paths.insert("重做换了话");
        }
        let appended = appended_of(actions);
        assert!(
            matches!(appended.first(), Some(event) if event.body == Body::TurnReverted(TurnReverted { turns: vec![turn] }) && event.by == alice()),
            "种子 {seed}：重做先记一条撤最后一轮的撤销：{actions:?}"
        );
        let recalls: Vec<&Vec<ContentHash>> = actions
            .iter()
            .filter_map(|action| match action {
                Action::Recall { blobs } => Some(blobs),
                _ => None,
            })
            .collect();
        assert!(
            match recall.is_empty() {
                true => recalls.is_empty(),
                false => recalls == [&recall.to_vec()],
            },
            "种子 {seed}：取回原文要 {recall:?}：{actions:?}"
        );
        // 撤销还没记进看守（`undo_check` 在这之后）：照撤销的规矩算它会拿走的，除了这一轮中途来的，就是要重发的。
        let taken = self.undone_by(&[turn]);
        resend.said = taken
            .into_iter()
            .filter(|seq| {
                self.find(*seq).is_some_and(|event| {
                    event.turn != Some(turn)
                        && matches!(event.body, Body::MessageUser(_))
                        && matches!(event.by, By::Person(_))
                })
            })
            .collect();
        if resend.said.len() > 1 {
            self.seen_paths.insert("重做重发了几句");
        }
        if actions
            .iter()
            .any(|action| matches!(action, Action::Restore { .. }))
        {
            self.seen_paths.insert("重做先改回文件");
            assert_eq!(appended.len(), 1, "种子 {seed}：改回文件以前只记撤销");
            self.redoing.restoring = Some(resend);
            return;
        }
        self.resent(&appended[1..], &resend);
    }

    /// 改回文件的结局回来了：在等的是重做的，结局后面重发（`watch/restore.rs` 查结局那一条）。
    pub(super) fn redo_restored(&mut self, actions: &[Action], recorded: bool) {
        if !recorded {
            return;
        }
        let Some(resend) = self.redoing.restoring.take() else {
            return;
        };
        let appended = appended_of(actions);
        self.resent(&appended[1..], &resend);
    }

    /// 重发的那一段：原来那几句一句一条，照先后，内容照原来的（开这一轮的那一句换成交来的，原来文字以外的块接在后面），
    /// `by` 照原来的，`cause` 是这个命令，不带回合编号；紧跟着由最后一句开一轮，`cause` 也是这个命令。
    fn resent(&self, events: &[&Event], resend: &Resend) {
        let seed = self.seed;
        let n = resend.said.len();
        assert!(
            n > 0 && events.len() > n,
            "种子 {seed}：重做重发撤掉的人的话，再开一轮：{events:?}"
        );
        for (event, seq) in events.iter().zip(&resend.said) {
            let original = self.find(*seq).unwrap();
            let Body::MessageUser(MessageUser {
                blocks,
                venue: None,
            }) = &original.body
            else {
                panic!("种子 {seed}：要重发的 {seq} 不是人的话");
            };
            let expected = match Some(*seq) == resend.opener {
                true => edited(blocks, &resend.change),
                false => blocks.clone(),
            };
            assert_eq!(
                event.body,
                Body::MessageUser(MessageUser {
                    blocks: expected,
                    venue: None
                }),
                "种子 {seed}：重发的内容照原来的"
            );
            assert_eq!(
                (&event.by, event.cause.as_ref(), event.turn),
                (&original.by, Some(&resend.command), None),
                "种子 {seed}：重发的 by 照原来的，cause 是重做的命令，不带回合编号"
            );
        }
        let last = events[n - 1].seq;
        assert!(
            matches!(&events[n].body, Body::TurnStarted(started) if started.trigger == Some(last))
                && events[n].cause.as_ref() == Some(&resend.command),
            "种子 {seed}：由重发的最后一句开一轮：{events:?}"
        );
    }
}

/// 开这一轮的那一句照 `change` 换过的样子（`redo.rs` 的 `Redo::edited`）：两样都没换的原样；换了的，字（没换的照原来的
/// 文字块）在前、附件（没换的照原来文字以外的块）在后。
fn edited(original: &[Block], change: &Change) -> Vec<Block> {
    let (text, attachments) = change;
    if text.is_none() && attachments.is_none() {
        return original.to_vec();
    }
    let is_text = |block: &&Block| matches!(block, Block::Text(Text { .. }));
    let words: Vec<Block> = match text {
        Some(text) => text.clone(),
        None => original.iter().filter(is_text).cloned().collect(),
    };
    let attached: Vec<Block> = match attachments {
        Some(attachments) => attachments.clone(),
        None => original
            .iter()
            .filter(|block| !is_text(block))
            .cloned()
            .collect(),
    };
    words.into_iter().chain(attached).collect()
}

/// 这一串动作追加的事件，照先后。读回日志时到的转述（施工 8-17）先放着、读完才记，排在这一批后面，不算重做的。
fn appended_of(actions: &[Action]) -> Vec<&Event> {
    actions
        .iter()
        .filter_map(|action| match action {
            Action::Append(events) => Some(events),
            _ => None,
        })
        .flatten()
        .filter(|event| !matches!(event.body, Body::ImageDescribed(_)))
        .collect()
}
