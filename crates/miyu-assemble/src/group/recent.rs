//! 群聊近况（施工 O-13 下，`docs/designs/18-通讯平台.md` 第九节，`docs/construction/O-13-群聊近况（下）.md`）：开一轮的那条人的
//! 消息前面的一块，收上一个这样的触发之后、这一条之前的旁听消息（睡着时收到的不收）和别的线替她发进群里的话，一行一条。
//!
//! 只看这一条以前的日志：撤回的标记只认这以前记下的，这一轮开了以后哪几条、写什么就定了，以后每次请求一字不差。从最新往
//! 前装，装不下的写一行缺口提示。

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use miyu_kernel::event::{Body, Event};
use miyu_kernel::history::History;
use miyu_kernel::id::{ExternalId, Seq};

use super::{recall_mark, record, you};
use crate::texts::GroupChat;

/// 第 `before` 条开始一轮，上一个由人的消息开的回合从第 `after` 条开始（没有的是没有）：这一块近况，当过触发的
/// （`answered`）不收。一条都没有、快照里没有近况的字的，
/// 没有。
pub(crate) fn recent(
    history: &History,
    after: Option<Seq>,
    before: Seq,
    answered: &BTreeSet<Seq>,
    texts: &GroupChat,
) -> Option<String> {
    let words = texts.recent.as_ref()?;
    // 撤回的只认这一条以前记下的：之后才撤的，已经发过的这一块不改。
    let recalled: BTreeMap<&str, &ExternalId> = history
        .events()
        .iter()
        .take_while(|event| event.seq < before)
        .filter_map(|event| match &event.body {
            Body::VenueRecalled(recalled) => Some((recalled.msg.as_str(), &recalled.by)),
            _ => None,
        })
        .collect();
    let lines: Vec<String> = history
        .events()
        .iter()
        .filter(|event| after.is_none_or(|after| event.seq > after) && event.seq < before)
        .filter(|event| !answered.contains(&event.seq))
        .filter_map(|event| one(history, event, &recalled, texts))
        .collect();
    // 从最新往前装，装不下就停。
    let mut kept = VecDeque::new();
    let mut used = 0;
    for line in lines.iter().rev() {
        used += line.len() + 1;
        if used > words.budget {
            break;
        }
        kept.push_front(line.as_str());
    }
    if kept.is_empty() {
        return None;
    }
    let mut block = words.open.clone();
    let left = lines.len() - kept.len();
    if left > 0 {
        let count = left.to_string();
        let fields = BTreeMap::from([("count", count.as_str())]);
        // 造快照时试换过，这里换不出只会是造的时候没查到的 bug，照空的写。
        block.push_str(&words.omitted.render(&fields).unwrap_or_default());
    }
    for line in kept {
        block.push_str(line);
        block.push('\n');
    }
    Some(block)
}

/// 近况里的一条：旁听的人的话（睡着时收到的不要），别的线替她发进群里的话（这条线自己的不要）；别的不算。
fn one(
    history: &History,
    event: &Event,
    recalled: &BTreeMap<&str, &ExternalId>,
    texts: &GroupChat,
) -> Option<String> {
    match &event.body {
        Body::MessageUser(message) => {
            let venue = message
                .venue
                .as_ref()
                .filter(|venue| venue.ambient && !venue.asleep)?;
            let mark = recalled
                .get(venue.msg.as_str())
                .map(|by| recall_mark(&event.by, by, venue.show_ids))
                .unwrap_or_default();
            Some(record(
                event.at,
                &event.by,
                venue,
                &message.blocks,
                texts,
                &mark,
            ))
        }
        Body::VenueDelivered(delivered) if history.own() != Some(&delivered.line) => {
            Some(you(event, delivered, texts))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests;
