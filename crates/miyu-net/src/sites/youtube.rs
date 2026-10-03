//! YouTube（`net.md`「怎么走」第 12 条第 2 款，W-7 再补）：读页面读过 `</head>`（`body.rs` 的 `Until::Video`），
//! 挖元数据整段都看；时长照 `<meta itemprop="duration">`，频道名照 `<link itemprop="name">`。读到了时长的才是视频。

use super::Draft;
use crate::Kind;
use crate::html;

/// 从读到的整段里补上时长、频道名、是不是视频。
pub(super) fn fill(text: &str, draft: &mut Draft) {
    draft.duration =
        html::first(text, "meta", "itemprop", "duration", "content").and_then(|iso| seconds(&iso));
    draft.author = html::first(text, "link", "itemprop", "name", "content").unwrap_or_default();
    if draft.duration.is_some() {
        draft.kind = Kind::Video;
    }
}

/// ISO 8601 的时长换成秒：`P`，可选的天，再可选的 `T` 加时、分、秒，都是整数（`PT3M33S`、`PT1H2M3S`、`P1DT2H`）。
/// 读不了的、是 0 的交 `None`。
pub(super) fn seconds(iso: &str) -> Option<u64> {
    let rest = iso.trim().strip_prefix('P')?;
    let (days, time) = match rest.split_once('T') {
        Some((days, time)) => (days, Some(time)),
        None => (rest, None),
    };
    let mut total = units(days, &[('D', 86_400)])?;
    if let Some(time) = time {
        if time.is_empty() {
            return None;
        }
        total = total.checked_add(units(time, &[('H', 3_600), ('M', 60), ('S', 1)])?)?;
    }
    (total > 0).then_some(total)
}

/// 一截「数字加单位」排成的字，单位只能照 `allowed` 的先后各出现一次；交回合起来的秒数。空的是 0。
fn units(text: &str, allowed: &[(char, u64)]) -> Option<u64> {
    let mut total: u64 = 0;
    let mut digits = String::new();
    let mut next = 0;
    for c in text.chars() {
        if c.is_ascii_digit() {
            digits.push(c);
            continue;
        }
        let at = allowed[next..].iter().position(|(unit, _)| *unit == c)? + next;
        if digits.is_empty() {
            return None;
        }
        let value: u64 = digits.parse().ok()?;
        total = total.checked_add(value.checked_mul(allowed[at].1)?)?;
        digits.clear();
        next = at + 1;
    }
    digits.is_empty().then_some(total)
}

#[cfg(test)]
mod tests;
