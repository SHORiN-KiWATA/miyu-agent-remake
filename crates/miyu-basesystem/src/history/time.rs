//! `since`、`until` 的写法（`docs/blueprint/tools/history.md`「参数」，施工 6-4）：照这个会话的时区，
//! `2026-09-29 14:00`，或者只写日期 `2026-09-29`。日期和钟点之间写 `T` 的也认，秒写了不看。
//!
//! 范围是左闭右开的：`since` 从那一分钟（只写日期的，那一天的 0 点）起；`until` 到那一分钟完（只写日期的，到那一天
//! 的 24 点）。

use std::ops::RangeInclusive;

use miyu_kernel::time::{Timestamp, UtcOffset};

/// 一天多少毫秒。
const DAY_MS: i64 = 24 * 60 * 60 * 1000;
/// 一分钟多少毫秒。
const MINUTE_MS: i64 = 60 * 1000;

/// 读出来的一个时刻：那一分钟（或者那一天）从哪一刻开始、到哪一刻为止（不含）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Span {
    pub(super) start: Timestamp,
    pub(super) end: Timestamp,
}

/// 读 `text`，照时区 `offset`。写得不对的、日期或者钟点不存在的，没有。
pub(super) fn parse(text: &str, offset: UtcOffset) -> Option<Span> {
    let text = text.trim();
    let (date, clock) = match text.find([' ', 'T']) {
        Some(at) => (&text[..at], Some(text[at + 1..].trim_start())),
        None => (text, None),
    };
    let [year, month, day] = numbers(date, '-', [4..=4, 1..=2, 1..=2])?;
    let (hour, minute, width) = match clock {
        Some(clock) => {
            let clock = clock.split(':').take(2).collect::<Vec<_>>().join(":");
            let [hour, minute] = numbers(&clock, ':', [1..=2, 2..=2])?;
            (hour, minute, MINUTE_MS)
        }
        None => (0, 0, DAY_MS),
    };
    let start = Timestamp::from_local(year, month, day, hour, minute, offset)?;
    let end = Timestamp::from_unix_millis(start.unix_millis() + width)?;
    Some(Span { start, end })
}

/// 照 `separator` 拆成几段数字，每一段的位数在 `widths` 里（月、日、钟点写一位的也认）。
fn numbers<const N: usize>(
    text: &str,
    separator: char,
    widths: [RangeInclusive<usize>; N],
) -> Option<[i64; N]> {
    let parts: Vec<&str> = text.split(separator).collect();
    if parts.len() != N {
        return None;
    }
    let mut out = [0; N];
    for (k, (part, width)) in parts.iter().zip(widths).enumerate() {
        if !width.contains(&part.len()) || !part.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        out[k] = part.parse().ok()?;
    }
    Some(out)
}
