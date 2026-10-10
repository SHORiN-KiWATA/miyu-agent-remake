//! 订阅补来的（施工 O-32，`onebot.md` 第一条「群里怎么叫她」第 1 条、「出站队列」第 6 条）：桥订阅一个场所会话、补日志
//! （`after`）的那一段里，认出发到哪、补发哪几句。纯逻辑：期限照此刻算好交进来（`since`），不碰 I/O、时钟。事件照原样的
//! JSON 看，群里、私聊一个看法。
//!
//! - 发到哪：场所照日志第一条 `session.created` 的 `venue`；机器人号照最近一条人说的话的命令编号（`cause`，
//!   `qq:<机器人的号>:…`，施工单「要定的」第 3 条）。认不出的等那个场所来一条消息。
//! - 补发哪几句：补来的她的话（这一轮说了不说话的不算，调的一方认），说的时刻晚于 `since`：还在出站队列的期限里（施工单
//!   「要定的」第 1 条）。照同一个回合入队了的正文比（`ext.onebot.venues.queued` 里 `kind` 是 `reply` 的，同 O-25 中的去重，
//!   「要定的」第 2 条）：出站链照这一回合入队了的去重，拆出来的每一段入队过的不再发。
//!
//! 只在订阅到补完的这一会儿有（`Route` 的 `backlogs`）：补完了交给 `revive` 补发，丢掉。

use std::collections::HashMap;

use miyu_chat::Venue;
use miyu_kernel::id::VenueId;
use miyu_kernel::time::Timestamp;
use serde_json::Value;

use super::projection::{QUEUED, REPLY, Speaking};
use crate::onebot::{To, bot_of, to_of};

/// 订阅补来的那一段。
#[derive(Debug, Default)]
pub(super) struct Backlog {
    /// 补到哪（`subscribe` 回应的 `upto`）：序号不大于它的是补来的。
    upto: u64,
    /// 说的时刻晚于它（毫秒）的还在期限里。
    since: i64,
    /// 场所：`session.created` 的 `venue`。
    venue: Option<VenueId>,
    /// 机器人号：最近一条人说的话的命令编号认出来的。
    bot: Option<i64>,
    /// 回合编号 → 这一回合入队了的她的话的正文，照先后。
    queued: HashMap<u64, Vec<String>>,
    /// 期限以内的她的话，照先后。
    said: Vec<Said>,
}

/// 补来的她的一句：期限以内、这一轮没说不说话。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Said {
    /// 回合编号。
    pub(super) turn: u64,
    /// 她说的字（`text` 块接起来的）。
    pub(super) text: String,
    /// 群里的：她说它那一刻群里的样子（投影交出的，出站链要的）；私聊的是空的。
    pub(super) speaking: Option<Speaking>,
}

impl Backlog {
    /// 补到 `upto`，说的时刻晚于 `since`（毫秒）的还在期限里。
    pub(super) fn new(upto: u64, since: i64) -> Backlog {
        Backlog {
            upto,
            since,
            ..Backlog::default()
        }
    }

    /// 事件 `event` 是不是补来的：序号不大于 `upto`。没有序号的（瞬时的）不是。
    pub(super) fn replayed(&self, event: &Value) -> bool {
        event["seq"].as_u64().is_some_and(|seq| seq <= self.upto)
    }

    /// 收一条补来的事件：认场所、机器人号、这一回合入队了的。
    pub(super) fn take(&mut self, event: &Value) {
        match event["kind"].as_str() {
            Some("session.created") => {
                self.venue = event["body"]["venue"]
                    .as_str()
                    .and_then(|venue| VenueId::parse(venue).ok());
            }
            Some("message.user") => {
                if let Some(bot) = event["cause"].as_str().and_then(bot_of) {
                    self.bot = Some(bot);
                }
            }
            _ => {
                if let Some((turn, text)) = queued_reply(event) {
                    self.queued.entry(turn).or_default().push(text);
                }
            }
        }
    }

    /// 补来的她的一句 `event`，字是 `text`，群里那一刻的样子是 `speaking`：说的时刻还在期限里的记下。没有回合编号的、时刻读不出
    /// 的（照说不会）不记。
    pub(super) fn said(&mut self, event: &Value, text: String, speaking: Option<Speaking>) {
        let Some(turn) = event["turn"].as_u64() else {
            return;
        };
        let at = event["at"]
            .as_str()
            .and_then(|at| Timestamp::parse(at).ok());
        if at.is_some_and(|at| at.unix_millis() > self.since) {
            self.said.push(Said {
                turn,
                text,
                speaking,
            });
        }
    }

    /// 发到哪：机器人号、发给谁、场所。场所、机器人号有一样认不出的是空的。
    pub(super) fn peer(&self) -> Option<(i64, To, VenueId)> {
        let venue = self.venue.clone()?;
        let to = to_of(&Venue::parse(&venue)?)?;
        Some((self.bot?, to, venue))
    }

    /// 期限以内的她的话，照先后取出来。
    pub(super) fn take_said(&mut self) -> Vec<Said> {
        std::mem::take(&mut self.said)
    }

    /// 回合编号是 `turn` 的那一回合入队了的正文，照先后。
    pub(super) fn sent(&self, turn: u64) -> Vec<String> {
        self.queued.get(&turn).cloned().unwrap_or_default()
    }

    /// 回合编号是 `turn` 的一句拆出来的几段 `pieces`：这一回合入队过的去掉；留下的算进这一回合入队了的（同一回合后面的几句照它
    /// 去重），照先后交回。
    pub(super) fn unsent(&mut self, turn: u64, pieces: Vec<String>) -> Vec<String> {
        let queued = self.queued.entry(turn).or_default();
        let fresh: Vec<String> = pieces
            .into_iter()
            .filter(|piece| !queued.contains(piece))
            .collect();
        queued.extend(fresh.iter().cloned());
        fresh
    }
}

/// 一条入队（`ext.onebot.venues.queued`）是她的话的：（回合编号，正文）；别的是空的。
pub(super) fn queued_reply(event: &Value) -> Option<(u64, String)> {
    let body = &event["body"];
    if event["kind"] != QUEUED || body["kind"] != REPLY {
        return None;
    }
    Some((body["turn"].as_u64()?, body["text"].as_str()?.to_string()))
}

#[cfg(test)]
mod tests;
