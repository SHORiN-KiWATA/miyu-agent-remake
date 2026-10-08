//! 回合开始的那一组（从 `render.rs` 挪出来，那边放不下了）：由人的消息开的回合各在哪一条开始、哪些旁听的当过触发（[`Opened`]）；
//! 群聊近况放在事实后面、触发前面（施工 O-13 下、O-14 上），照记下的几条开的回合，那几条在回合开始的地方渲染（施工 O-14 上）。

use std::collections::BTreeSet;

use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, Event, MessageUser, TurnStarted};
use miyu_kernel::history::History;
use miyu_kernel::id::Seq;

use super::{Place, Transcript, known, spoken, text_block};
use crate::group;
use crate::texts::Texts;

/// 由第 `trigger` 条开的回合开始了（`started` 是 `event` 的内容）：记下开始的地方。群会话里由人的消息开的一轮，先放群聊近况
/// （施工 O-13 下），在回合开始那一组里单占一格，排在事实后面、触发前面（施工 O-14 上）；照记下的几条开的（施工 O-14 上），
/// 那几条旁听的话在这里渲染，不在它们自己的位置：以前的请求里没有它们。
pub(super) fn opening(
    transcript: &mut Transcript,
    history: &History,
    event: &Event,
    started: &TurnStarted,
    opened: &Opened,
    texts: &Texts,
) {
    let Some(trigger) = started.trigger else {
        return;
    };
    transcript.start(event.turn, trigger);
    let recent = texts
        .group
        .as_ref()
        .zip(opened.window(event.seq))
        .and_then(|(chat, after)| group::recent(history, after, event.seq, &opened.answered, chat));
    transcript.add(
        trigger,
        Place::Lead(trigger),
        recent.into_iter().map(text_block).collect(),
    );
    transcript.add(
        trigger,
        Place::Here,
        joined(history, &started.triggers, texts),
    );
}

/// 照记下的几条开的、并进来的（`triggers`）那几条旁听的话渲染成的块，照序号的先后（施工 O-14 上、下）。
pub(super) fn joined(history: &History, triggers: &[Seq], texts: &Texts) -> Vec<Block> {
    triggers
        .iter()
        .filter_map(|seq| message_at(history, *seq))
        .flat_map(|(said, message)| spoken(history, said, message, known(&message.blocks), texts))
        .collect()
}

/// 由人的消息开的回合（施工 O-13 下）：`session.send` 开的、照记下的几条开的（施工 O-14 上），各在哪一条开始；群聊近况收
/// 两次这样的回合开始之间的。手动压缩单开的那一轮、回报开的回合不算。当过触发的（`triggers`）不进近况。
pub(super) struct Opened {
    /// 这样的回合的 `turn.started`，照先后。
    starts: Vec<Seq>,
    /// 当过触发的旁听消息。
    answered: BTreeSet<Seq>,
}

impl Opened {
    pub(super) fn of(history: &History) -> Opened {
        let said: BTreeSet<Seq> = history
            .events()
            .iter()
            .filter(|event| matches!(event.body, Body::MessageUser(_)))
            .map(|event| event.seq)
            .collect();
        let mut opened = Opened {
            starts: Vec::new(),
            answered: BTreeSet::new(),
        };
        for event in history.events() {
            match &event.body {
                Body::TurnStarted(started)
                    if started
                        .trigger
                        .is_some_and(|trigger| said.contains(&trigger)) =>
                {
                    opened.starts.push(event.seq);
                    opened.answered.extend(&started.triggers);
                }
                // 并进正在跑的一轮的（施工 O-14 下）也当过触发。
                Body::TurnJoined(joined) => opened.answered.extend(&joined.triggers),
                _ => {}
            }
        }
        opened
    }

    /// 第 `start` 条开始的回合是这样的回合的，交回上一个这样的回合开始的那一条（没有的是没有）；不是的没有。
    fn window(&self, start: Seq) -> Option<Option<Seq>> {
        let at = self.starts.binary_search(&start).ok()?;
        Some(at.checked_sub(1).map(|before| self.starts[before]))
    }
}

/// 第 `seq` 条，是 `message.user` 的交回它和它的内容。
fn message_at(history: &History, seq: Seq) -> Option<(&Event, &MessageUser)> {
    let events = history.events();
    let at = events.binary_search_by_key(&seq, |event| event.seq).ok()?;
    match &events[at].body {
        Body::MessageUser(message) => Some((&events[at], message)),
        _ => None,
    }
}
