//! 场所规则套到这一条消息上（`onebot.md` 第一条「群消息」第 3、4、6 条，施工 O-22）：套好的样子（`rules::Applied`）里这一步
//! 用到的几项。写法、怎么套由群聊内核管（`chat.md` 第一条），这里只照键取值。
//!
//! - `venue.session` 带的人格、预设、工作区（`persona`、`preset`、`workspace` → `cwd`），没设的不带。
//! - 发的人的身份：`managers` 里有的是 `manager`，别的 `member`；QQ 的群主、管理员不算（「施工时定的」第 58 条）。
//! - `show_ids`。
//! - 此刻睡没睡：照群聊内核的回合闸问（`gate()`），只交睡眠，闸说推迟就是睡着；时区照本机此刻的偏移（「施工时定的」
//!   第 65 条）。
//! - 进站链要的（施工 O-23，「群里怎么叫她」第 4、5 条）：限流、睡眠、能不能叫她、触发词，此刻（本机的钟和时区）。
//! - 线路规程（施工 O-23 下，第 10 条）：`discipline` 的写法，没设的是空的。

use std::time::{SystemTime, UNIX_EPOCH};

use miyu_chat::{Clock, Ctx, Gate, Moderation, Rate, Sleep, gate};
use miyu_config::Value;
use miyu_kernel::id::ExternalId;
use miyu_kernel::time::{Timestamp, UtcOffset};

use crate::rules::Applied;

/// `venue.session` 的格和场所规则里对着的键（「群消息」第 3 条）。
const OPENING: [(&str, &str); 3] = [
    ("persona", "persona"),
    ("preset", "preset"),
    ("cwd", "workspace"),
];

/// `venue.session` 要带的几格：格名和值，照 [`OPENING`] 的先后，规则没设的不在里面。
pub(super) fn opening(applied: &Applied) -> Vec<(&'static str, &str)> {
    OPENING
        .iter()
        .filter_map(|(field, key)| text(applied, key).map(|value| (*field, value)))
        .collect()
}

/// 平台上是 `who` 的人在这个场所里的身份：规则的 `managers` 里有的是 `manager`，别的 `member`（「群消息」第 4 条）。
pub(super) fn role(applied: &Applied, who: &ExternalId) -> &'static str {
    let listed = match entry(applied, "managers") {
        Some(Value::List(managers)) => managers
            .iter()
            .any(|manager| matches!(manager, Value::Text(id) if id == who.as_str())),
        _ => false,
    };
    match listed {
        true => "manager",
        false => "member",
    }
}

/// 规则的 `show_ids` 是不是真的。
pub(super) fn show_ids(applied: &Applied) -> bool {
    matches!(entry(applied, "show_ids"), Some(Value::Bool(true)))
}

/// 此刻落在规则的睡觉时间里没有：没设、写成 `off` 的醒着。照群聊内核的回合闸问：只交睡眠（不限流、没禁言），闸说推迟就是
/// 睡着。本机的钟读不出（早于 1970 年、晚于 9999 年）的当醒着。
pub(super) fn asleep(applied: &Applied) -> bool {
    let (Some(sleep), Some(clock)) = (sleep(applied), clock()) else {
        return false;
    };
    let ctx = Ctx {
        rate: None,
        sleep: Some(sleep),
        allow: None,
        muted: false,
        turns: Vec::new(),
        notices: Vec::new(),
        moderation: Moderation {
            keywords: Vec::new(),
            base64: applied.params.base64,
        },
    };
    matches!(gate(&ctx, clock), Gate::Later(_))
}

/// 规则的限流（施工 O-23）：没设、`"0"`、写错的是空的（[`Rate::read`]）。
pub(super) fn rate(applied: &Applied) -> Option<Rate> {
    text(applied, "rate").and_then(Rate::read)
}

/// 规则的睡觉时间：没设、`off`、写错的是空的（[`Sleep::read`]）。
pub(super) fn sleep(applied: &Applied) -> Option<Sleep> {
    text(applied, "sleep").and_then(Sleep::read)
}

/// 规则的 `allow`（施工 O-23）：没设的是空的，进站链当能叫她。
pub(super) fn allow(applied: &Applied) -> Option<bool> {
    match entry(applied, "allow") {
        Some(Value::Bool(allow)) => Some(*allow),
        _ => None,
    }
}

/// 规则的触发词 `keywords`（施工 O-23，「群里怎么叫她」第 4 条）：没设的是空的。
pub(super) fn keywords(applied: &Applied) -> Vec<String> {
    match entry(applied, "keywords") {
        Some(Value::List(keywords)) => keywords
            .iter()
            .filter_map(|keyword| match keyword {
                Value::Text(keyword) => Some(keyword.to_string()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// 规则的线路规程 `discipline`（施工 O-23 下）：没设的是空的。
pub(super) fn discipline(applied: &Applied) -> Option<&str> {
    text(applied, "discipline")
}

/// 此刻：本机的钟，时区照本机此刻的偏移（「施工时定的」第 65 条）。钟读不出（早于 1970 年、晚于 9999 年）的是空的。
pub(super) fn clock() -> Option<Clock> {
    Some(Clock {
        now: now()?,
        offset: offset(),
    })
}

/// 规则设到的一项：没有规则设到的是空的。
fn entry<'a>(applied: &'a Applied, key: &str) -> Option<&'a Value> {
    applied.resolved.entries.get(key).map(|entry| &entry.value)
}

/// 规则设到的一项字（名字、文字、睡眠的写法）。
fn text<'a>(applied: &'a Applied, key: &str) -> Option<&'a str> {
    match entry(applied, key) {
        Some(Value::Text(text)) => Some(text),
        _ => None,
    }
}

/// 此刻，照本机的钟。
fn now() -> Option<Timestamp> {
    let since = SystemTime::now().duration_since(UNIX_EPOCH).ok()?;
    Timestamp::from_unix_millis(i64::try_from(since.as_millis()).ok()?)
}

/// 本机此刻的时区偏移，到分钟（照核心给会话钉下时区的算法，`miyu-endpoint` 的 `sessions.rs`）。读出来超了范围的当 UTC。
fn offset() -> UtcOffset {
    let minutes = jiff::Zoned::now().offset().seconds() / 60;
    UtcOffset::from_minutes(minutes).unwrap_or(UtcOffset::UTC)
}

#[cfg(test)]
mod tests;
