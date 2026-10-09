//! 重做最后一轮（`docs/blueprint/kernel/history.md`「重做」，施工 4-7 再补；2026-09-30 项目主人定：核心一个命令，只重做
//! 最后一轮）：撤掉它，把开它的那几句人的话原样再发一次（开这一轮的那一句也可以换掉字、换掉附件），开新的一轮。
//!
//! 一个命令做完，不会撤成了、没发出去：撤销那一半照撤销的规矩走（改回文件、撤掉压缩时读回日志，`revert.rs`），撤销
//! （改回文件的结局）记下的那一批里接着重发、开新的一轮。重发的是撤销照「拿走什么」拿走的人的话里，不带那一轮编号的
//! 几句：排着接过来的，和开这一轮的那一句。新的一轮开了，撤销就恢复不了。
//!
//! 最后一轮不是人说的话开的（回报叫醒的、另一个会话的话开的、手动压缩、清空单开的、重启以后接着干的），或者一轮都
//! 没有：拒绝，`not_redoable`，一个原因码管两种。换过的那一句一块都不剩：拒绝，`empty_message`。

use super::action::{Action, Reason};
use super::{Session, rejected};
use crate::block::Block;
use crate::event::{Body, Event, MessageUser};
use crate::id::{CommandId, Seq, TurnId};
use crate::origin::By;
use crate::time::Timestamp;

/// 重做的撤销记下以后要做的：撤的是哪一轮、开这一轮的那一句换成什么。撤销那一半在读回日志、改回文件的时候，它跟着
/// 那一段等着（`revert.rs` 的 `ReadingBack`、`Restoring`）。
#[derive(Debug)]
pub(super) struct Redo {
    /// 撤的是哪一轮：最后一轮。
    pub(super) turn: TurnId,
    /// 开这一轮的那一句里的字换成的块；`None` 是照原来的。
    text: Option<Vec<Block>>,
    /// 开这一轮的那一句里的附件换成的块；`None` 是照原来的。
    attachments: Option<Vec<Block>>,
}

/// 开回合 `turn` 的那一句，照有效历史看。
pub(super) enum Opener<'a> {
    /// 人亲口说的 `message.user`：能重做。
    Person(&'a Event),
    /// 没有触发，或者触发的不是人说的话：不能重做。
    Not,
    /// 那一轮的开头、触发的那一条不在这里面（压缩掉了）：说不准。
    Unknown,
}

impl Redo {
    /// 这一次重做能不能照 `events`（照序号排好的有效历史）做：能的没有，不能的交回原因码。开这一轮的那一句不是人说的话、
    /// 说不准的，`not_redoable`；换过以后一块都不剩的，`empty_message`。
    pub(super) fn refusal(&self, events: &[Event]) -> Option<Reason> {
        match opener(events, self.turn) {
            Opener::Person(event) if self.edited(said(event)).is_empty() => {
                Some(Reason::EmptyMessage)
            }
            Opener::Person(_) => None,
            Opener::Not | Opener::Unknown => Some(Reason::NotRedoable),
        }
    }

    /// 开这一句换过的样子：字照 `text` 换（没有的照原来的文字块），附件照 `attachments` 换（没有的照原来文字以外的块），
    /// 字在前、附件在后，和 `session.send` 写的一样。两样都没换的，原来那一句原样。
    fn edited(&self, original: &[Block]) -> Vec<Block> {
        if self.text.is_none() && self.attachments.is_none() {
            return original.to_vec();
        }
        let text = |block: &&Block| matches!(block, Block::Text(_));
        let words = match &self.text {
            Some(text) => text.clone(),
            None => original.iter().filter(text).cloned().collect(),
        };
        let attached = match &self.attachments {
            Some(attachments) => attachments.clone(),
            None => original
                .iter()
                .filter(|block| !text(block))
                .cloned()
                .collect(),
        };
        words.into_iter().chain(attached).collect()
    }
}

impl Session {
    /// 重做最后一轮。有回合在进行的，`turn_running`；一轮都没有、最后一轮不是人说的话开的，`not_redoable`；换过的那一句
    /// 一块都不剩的，`empty_message`：都当场拒绝，什么都不记。
    ///
    /// 能不能重做照有效历史看：那一轮的开头、触发的那一条都在的当场判。它们压缩掉了的（那一轮中途压过），撤它反正要
    /// 读回日志，读回来再判（[`Session::read_back`]）。撤销照撤销的规矩记，`turns` 只有这一轮、`by` 是重做的人、`cause`
    /// 是这个命令；重发和新的一轮跟在撤销、改回文件的结局后面（[`Session::resend`]）。
    pub(super) fn redo(
        &mut self,
        id: CommandId,
        by: By,
        at: Timestamp,
        text: Option<Vec<Block>>,
        attachments: Option<Vec<Block>>,
    ) -> Vec<Action> {
        if self.turn.is_some() {
            return vec![rejected(id, Reason::TurnRunning)];
        }
        let Some(turn) = self.ledger.last_turn() else {
            return vec![rejected(id, Reason::NotRedoable)];
        };
        let redo = Redo {
            turn,
            text,
            attachments,
        };
        let refusal = redo.refusal(self.history.events());
        let known = !matches!(opener(self.history.events(), turn), Opener::Unknown);
        match (refusal, self.ledger.read_back_from(turn)) {
            // 说不准的，读回来再判；开头、触发的那一条不在有效历史里，又不用读回：照账本不会有，当作重做不了，不猜。
            (Some(reason), from) if known || from.is_none() => vec![rejected(id, reason)],
            (_, Some(from)) => self.read_back_first(id, by, vec![turn], from, Some(redo)),
            (_, None) => self.reverted(id, by, at, vec![turn], Some(redo)),
        }
    }

    /// 重做的撤销记下了（有要改回的文件的，结局也记下了）：把撤掉的那几句人的话照先后再发一次，开这一轮的那一句照交来的
    /// 换过（[`Redo::edited`]），由它的新的那一条开新的一轮。交回追加的事件：重发的几句，新的一轮的开头那一批。
    ///
    /// 重发的是最近一次撤销拿走的人亲口说的 `message.user`，除了带着那一轮编号的（那一轮中途来的话，撤掉的那一轮的
    /// 第一次请求没听到过它们）：排着接过来的几句和开这一轮的那一句（`history.md`「拿走什么」）。内容块、`by` 照原来的，
    /// 附件跟着；`cause` 是这个命令，空闲时追加，不带回合编号。
    pub(super) fn resend(&mut self, at: Timestamp, id: &CommandId, redo: Redo) -> Vec<Event> {
        let undone = self.history.last_undone();
        let trigger = undone.iter().find_map(|event| match &event.body {
            Body::TurnStarted(started) if event.seq == redo.turn.started() => started.trigger,
            _ => None,
        });
        let said: Vec<(Seq, By, Vec<Block>)> = undone
            .iter()
            .filter(|event| event.turn != Some(redo.turn))
            .filter_map(|event| match (&event.body, &event.by) {
                (Body::MessageUser(message), By::Person(_)) => {
                    Some((event.seq, event.by.clone(), message.blocks.clone()))
                }
                _ => None,
            })
            .collect();
        let mut events = Vec::new();
        let mut opener = None;
        for (seq, by, blocks) in said {
            let opens = Some(seq) == trigger;
            let blocks = match opens {
                true => redo.edited(&blocks),
                false => blocks,
            };
            let body = Body::MessageUser(MessageUser {
                blocks,
                venue: None,
            });
            let message = self.record(at, by, Some(id.clone()), body);
            if opens {
                opener = Some(message.seq);
            }
            events.push(message);
        }
        // 开这一轮的那一句一定在拿走的里面（能不能重做就是照它判的），也一定排在最后。
        if let Some(opener) = opener {
            events.extend(self.open_turn(at, opener, Some(id.clone())));
        }
        events
    }
}

/// 开回合 `turn` 的那一句，照 `events`（照序号排好的有效历史）看：它的 `turn.started` 有触发，触发的那一条是人亲口说的
/// `message.user`（`by` 是有账号的人）的，交回那一条。开头、触发的那一条不在 `events` 里的（压缩掉了），说不准。
pub(super) fn opener(events: &[Event], turn: TurnId) -> Opener<'_> {
    let find = |seq: Seq| {
        events
            .binary_search_by_key(&seq, |event| event.seq)
            .ok()
            .map(|k| &events[k])
    };
    let Some(started) = find(turn.started()) else {
        return Opener::Unknown;
    };
    let Body::TurnStarted(started) = &started.body else {
        return Opener::Not;
    };
    let Some(trigger) = started.trigger else {
        return Opener::Not;
    };
    match find(trigger) {
        None => Opener::Unknown,
        Some(event)
            if matches!(event.body, Body::MessageUser(_)) && matches!(event.by, By::Person(_)) =>
        {
            Opener::Person(event)
        }
        Some(_) => Opener::Not,
    }
}

/// 一条 `message.user` 的内容块；别的没有。
fn said(event: &Event) -> &[Block] {
    match &event.body {
        Body::MessageUser(message) => &message.blocks,
        _ => &[],
    }
}
