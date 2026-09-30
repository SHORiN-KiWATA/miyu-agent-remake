//! 正文的钟（蓝图 `tui.md`「会话列表 `/sessions`」第 6 条）：看着流出来的照现在；补发来的照事件落盘的时刻
//! （[`Push::Clock`](crate::core::Push::Clock)），用时、时刻和当时一样。
//!
//! 事件的时刻是墙上的钟，用时是 [`Instant`]：补发开始时记下一对（这时的 `Instant`、这时的墙上时刻），事件的时刻
//! 往回倒推成 `Instant`，彼此的差和当时一样。

use std::time::Instant;

use jiff::{Timestamp, Zoned};

use super::Transcript;

/// 补发时照的时刻。
#[derive(Debug, Default, Clone, Copy)]
pub struct Clock {
    /// 正在读的那条事件的时刻；看着流出来的是 `None`。
    at: Option<Timestamp>,
    /// 这一次补发开始时的那一对。
    anchor: Option<(Instant, Timestamp)>,
}

impl Clock {
    /// 换成 `at` 的时刻；`None` 回到现在。
    pub fn set(&mut self, at: Option<Timestamp>) {
        self.at = at;
        self.anchor = match at {
            Some(_) => self
                .anchor
                .or_else(|| Some((Instant::now(), Timestamp::now()))),
            None => None,
        };
    }

    /// 现在（补发时是那条事件的时刻）。
    pub fn now(&self) -> Instant {
        let (Some(at), Some((base, wall))) = (self.at, self.anchor) else {
            return Instant::now();
        };
        let back = wall.duration_since(at);
        let gap = back.unsigned_abs();
        if back.is_negative() {
            base.checked_add(gap).unwrap_or(base)
        } else {
            base.checked_sub(gap).unwrap_or(base)
        }
    }

    /// 墙上的钟（收尾行写的时刻）。
    pub fn wall(&self) -> Zoned {
        self.at
            .map_or_else(Zoned::now, |at| at.to_zoned(jiff::tz::TimeZone::system()))
    }

    /// 在读补发来的事件。
    pub fn replaying(&self) -> bool {
        self.at.is_some()
    }
}

impl Transcript {
    /// 补发来的你说的话：照你说的画，记下序号（开轮时照它认，第 6 条）。
    pub(super) fn said(&mut self, seq: u64, text: String) {
        self.user(text, Vec::new());
        if let Some(entry) = self.entries.last_mut() {
            entry.seq = Some(seq);
        }
    }

    /// 正在读补发来的事件：不弹系统通知、不响、不报 herdr（第 6 条）。
    pub fn replaying(&self) -> bool {
        self.clock.replaying()
    }
}
