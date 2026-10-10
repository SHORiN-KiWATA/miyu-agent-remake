//! 出站队列的纯逻辑（施工 O-25 中，`onebot.md` 第一条「出站队列」第 3 到 8 条，「施工时定的」第 116 条）：每个场所会话一条先进
//! 先出的队，门开着的照先后交出来，排到了期限的作废（门关着的也是）；下一次该醒的时刻；禁言到什么时候；NapCat 的回应算成功
//! 还是失败、为什么。不碰 I/O、时钟：此刻、门开没开、禁言到什么时候都由调的一方交进来（`sending.rs`），停住的钟测得了。
//!
//! 门开没开、禁言到什么时候不记在这里：禁言照投影（日志算得出，桥重启照样重建），号连没连着照连着的号（「施工时定的」第 117 条）。

use std::collections::{BTreeMap, VecDeque};
use std::time::Duration;

use miyu_kernel::time::Timestamp;
use serde_json::Value;

use crate::onebot::{CallError, number};

/// `failed` 的 `why`：NapCat 回了失败。
pub(super) const REJECTED: &str = "rejected";

/// `failed` 的 `why`：等了调用的时限还没回。
pub(super) const TIMEOUT: &str = "timeout";

/// `failed` 的 `why`：写不进、等的时候连接断了。
pub(super) const DISCONNECTED: &str = "disconnected";

/// `failed` 的 `why`：排着过了期限（第 5 条）。
pub(super) const EXPIRED: &str = "expired";

/// `failed` 的 `detail` 最多几个字符（第 4 条）：NapCat 说的原因截到这么长。平台工具（一）答的原话也照它（施工 O-31）。
pub(super) const DETAIL: usize = 200;

/// 排着的一段：入队的时刻、要发的。
struct Waiting<T> {
    /// 入队的时刻（本机的钟，「施工时定的」第 118 条）。
    at: Timestamp,
    /// 要发的。
    item: T,
}

/// 出站队列：会话编号 → 排着的，照入队的先后。照会话编号排（`BTreeMap`）：看一遍的先后定得住。
pub(super) struct Queue<T> {
    /// 排着的过了多久作废，毫秒（`bridge.json` 的 `queue_expire_seconds`）。
    expire: i64,
    /// 会话编号 → 排着的；排空了的拿掉。
    lines: BTreeMap<String, VecDeque<Waiting<T>>>,
}

/// 看一遍一个会话交出来的：过了期的、门开着交得出去的，都照入队的先后。
#[derive(Debug)]
pub(super) struct Taken<T> {
    /// 过了期的：记 `expired`，不发。
    pub(super) expired: Vec<T>,
    /// 交出去的：门开着，没过期。
    pub(super) ready: Vec<T>,
}

impl<T> Queue<T> {
    /// 空的队：排着的过了 `expire` 作废（毫秒以下的不算；大得放不下的当一直不过期，照说不会）。
    pub(super) fn new(expire: Duration) -> Queue<T> {
        Queue {
            expire: i64::try_from(expire.as_millis()).unwrap_or(i64::MAX),
            lines: BTreeMap::new(),
        }
    }

    /// 会话 `session` 在 `at` 入队了一段 `item`：排在这个会话的最后。
    pub(super) fn push(&mut self, session: &str, at: Timestamp, item: T) {
        self.lines
            .entry(session.to_string())
            .or_default()
            .push_back(Waiting { at, item });
    }

    /// 有东西排着的会话，照编号排。
    pub(super) fn sessions(&self) -> Vec<String> {
        self.lines.keys().cloned().collect()
    }

    /// 看一遍会话 `session`，此刻是 `now`，门开着是 `open`：入队时刻加期限不晚于此刻的交出来作废（门关着的也是）；门开着的，
    /// 别的照先后交出去。门关着、没过期的留着。
    pub(super) fn take(&mut self, session: &str, now: Timestamp, open: bool) -> Taken<T> {
        let mut taken = Taken {
            expired: Vec::new(),
            ready: Vec::new(),
        };
        let expire = self.expire;
        let Some(line) = self.lines.get_mut(session) else {
            return taken;
        };
        let mut kept = VecDeque::new();
        for waiting in line.drain(..) {
            if deadline(&waiting, expire) <= now.unix_millis() {
                taken.expired.push(waiting.item);
            } else if open {
                taken.ready.push(waiting.item);
            } else {
                kept.push_back(waiting);
            }
        }
        if kept.is_empty() {
            self.lines.remove(session);
        } else {
            *line = kept;
        }
        taken
    }

    /// 下一次该看一遍的时刻（第 8 条）：排着的里最早的过期时刻；会话被禁言着的（`muted` 交回禁言到期的时刻，没禁言着的是空的）
    /// 禁言到期的时刻也算。没有排着的是空的：不用醒。
    pub(super) fn wake(&self, muted: impl Fn(&str) -> Option<Timestamp>) -> Option<Timestamp> {
        let deadlines = self
            .lines
            .values()
            .flatten()
            .map(|waiting| deadline(waiting, self.expire));
        let ends = self
            .lines
            .keys()
            .filter_map(|session| muted(session))
            .map(Timestamp::unix_millis);
        let first = deadlines.chain(ends).min()?;
        Timestamp::from_unix_millis(first)
    }
}

/// 排着的这一段到期的时刻，毫秒：入队的时刻加 `expire` 毫秒。
fn deadline<T>(waiting: &Waiting<T>, expire: i64) -> i64 {
    waiting.at.unix_millis().saturating_add(expire)
}

/// 禁言到什么时候（第 7 条，「施工时定的」第 122 条）：此刻 `now` 加 `seconds` 秒。算出来超出范围的是空的。
pub(super) fn until(now: Timestamp, seconds: u64) -> Option<Timestamp> {
    let millis = i64::try_from(seconds).ok()?.checked_mul(1000)?;
    Timestamp::from_unix_millis(now.unix_millis().checked_add(millis)?)
}

/// 交给 NapCat 的一段的结局（第 4 条）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Ending {
    /// 成了：NapCat 回的 `message_id`，没回的是空的。
    Sent(Option<i64>),
    /// 没成：`why`（[`REJECTED`]、[`TIMEOUT`]、[`DISCONNECTED`]），NapCat 说的原因（只有回了失败的有）。
    Failed(&'static str, Option<String>),
}

/// 一次调用的结果算成什么（第 4 条，「施工时定的」第 120 条）：回了 `ok` 的成了，带回的 `message_id`；回了失败的是
/// [`REJECTED`]，原因照回应的 `message` 去掉首尾空白、截到 [`DETAIL`] 个字符，空的、没有的不带；等不到是 [`TIMEOUT`]；
/// 连接断了是 [`DISCONNECTED`]。
pub(super) fn ending(result: &Result<Value, CallError>) -> Ending {
    match result {
        Ok(reply) => Ending::Sent(number(&reply["data"]["message_id"])),
        Err(CallError::Failed(reply)) => {
            let message = reply["message"].as_str().unwrap_or_default().trim();
            let detail = (!message.is_empty()).then(|| message.chars().take(DETAIL).collect());
            Ending::Failed(REJECTED, detail)
        }
        Err(CallError::Timeout) => Ending::Failed(TIMEOUT, None),
        Err(CallError::Closed) => Ending::Failed(DISCONNECTED, None),
    }
}

#[cfg(test)]
mod tests;
