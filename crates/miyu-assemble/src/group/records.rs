//! 判官看的群聊记录（施工 O-24，`docs/blueprint/chat.md` 第六条 `Ask.records`、`Ask.current`）：要判的那一条渲染成一行，它之前的
//! 群里的话一行一条，和她看到的同一个写法（18 第七节「判官和她各看各的」）。
//!
//! 收场所的 `message.user`（旁听的、开过回合的都收，睡着时收到的不收）和 `venue.delivered` 的 `[you]` 行（判官看不到她的回复，
//! 主线的也收）；撤回只认要判的那一条以前记下的。从新往旧取 `count` 条，照日志的先后排。

use std::collections::BTreeMap;

use miyu_kernel::event::Body;
use miyu_kernel::history::History;
use miyu_kernel::id::{ExternalId, Seq};

use super::{recall_mark, record, you};
use crate::texts::GroupChat;

/// 判官看的两样：之前的记录（一行一条，每行以换行结尾，没有的是空的）和要判的那一条。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Records {
    /// 之前的记录。
    pub records: String,
    /// 要判的那一条。
    pub current: String,
}

/// 第 `msg` 条之前至多 `count` 条群里的话，和第 `msg` 条那一行；它不是带 `venue` 的 `message.user` 的，没有。
pub fn records(history: &History, msg: Seq, count: usize, texts: &GroupChat) -> Option<Records> {
    let events = history.events();
    let at = events.binary_search_by_key(&msg, |event| event.seq).ok()?;
    let judged = &events[at];
    let Body::MessageUser(message) = &judged.body else {
        return None;
    };
    let venue = message.venue.as_ref()?;
    // 撤回的只认要判的那一条以前记下的。
    let recalled: BTreeMap<&str, &ExternalId> = events[..at]
        .iter()
        .filter_map(|event| match &event.body {
            Body::VenueRecalled(recalled) => Some((recalled.msg.as_str(), &recalled.by)),
            _ => None,
        })
        .collect();
    let lines: Vec<String> = events[..at]
        .iter()
        .filter_map(|event| match &event.body {
            Body::MessageUser(said) => {
                let venue = said.venue.as_ref().filter(|venue| !venue.asleep)?;
                let mark = recalled
                    .get(venue.msg.as_str())
                    .map(|by| recall_mark(&event.by, by, venue.show_ids))
                    .unwrap_or_default();
                Some(record(
                    event.at,
                    &event.by,
                    venue,
                    &said.blocks,
                    texts,
                    &mark,
                ))
            }
            Body::VenueDelivered(delivered) => Some(you(event, delivered, texts)),
            _ => None,
        })
        .collect();
    let mut kept = String::new();
    for line in &lines[lines.len().saturating_sub(count)..] {
        kept.push_str(line);
        kept.push('\n');
    }
    Some(Records {
        records: kept,
        current: record(judged.at, &judged.by, venue, &message.blocks, texts, ""),
    })
}

#[cfg(test)]
mod tests;
