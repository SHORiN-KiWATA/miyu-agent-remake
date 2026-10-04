//! 首页的吉祥物被列表顶上去以后待在哪（蓝图 `tui.md`「空会话的首页」第 4 条）：列表开着时只往上顶、
//! 不往下掉；上面放不下就不画，列表开着就一直不画；列表关了停一会儿，再一行一行走回原处。

use std::time::{Duration, Instant};

use super::model::Perch as Look;

/// 吉祥物现在在哪一行，和走下来的进度。
#[derive(Debug, Default)]
pub struct Perch {
    /// 被顶上去了：现在在第几行；在原处是 `None`。
    y: Option<i32>,
    /// 列表开着时放不下，不画。
    hidden: bool,
    /// 列表关了：关的时刻，和从第几行开始走。
    walk: Option<(Instant, i32)>,
    /// 刚被顶上去了一下，还没让嘴张（`take_hop`）。
    hopped: bool,
}

impl Perch {
    /// 这一刻画在第几行；不画是 `None`。`natural` 是不开列表时的那一行，`needed` 是开着列表时
    /// 最低能待在第几行（放不下是负的），没开列表是 `None`。
    pub fn place(
        &mut self,
        natural: u16,
        needed: Option<i32>,
        now: Instant,
        look: &Look,
    ) -> Option<u16> {
        let natural = i32::from(natural);
        if let Some(needed) = needed {
            self.walk = None;
            if self.hidden {
                return None;
            }
            let at = self.y.unwrap_or(natural).min(needed);
            // 往上挪了：像跳了一下，嘴张一下（「空会话的首页」第 9 条）。
            if at < self.y.unwrap_or(natural) {
                self.hopped = true;
            }
            if at < 0 {
                self.hidden = true;
                self.y = None;
                return None;
            }
            self.y = (at < natural).then_some(at);
            return u16::try_from(at).ok();
        }
        if !self.hidden && self.y.is_none() {
            return u16::try_from(natural).ok();
        }
        let (closed, start) = *self.walk.get_or_insert((now, self.y.unwrap_or(0)));
        let settle = closed + Duration::from_millis(look.settle_ms);
        if now < settle {
            return if self.hidden {
                None
            } else {
                u16::try_from(start).ok()
            };
        }
        let rows =
            now.saturating_duration_since(settle).as_millis() / u128::from(look.row_ms.max(1));
        let at = start.saturating_add(i32::try_from(rows).unwrap_or(i32::MAX));
        self.hidden = false;
        if at >= natural {
            self.y = None;
            self.walk = None;
            return u16::try_from(natural).ok();
        }
        self.y = Some(at);
        u16::try_from(at).ok()
    }

    /// 刚被顶上去过：交回一次，之后是 `false`。
    pub fn take_hop(&mut self) -> bool {
        std::mem::take(&mut self.hopped)
    }

    /// 正一行一行走下来（停着的那一会儿过完了）：嘴一路张着。
    pub fn walking(&self, now: Instant, look: &Look) -> bool {
        self.walk
            .is_some_and(|(closed, _)| now >= closed + Duration::from_millis(look.settle_ms))
    }

    /// 下一次该画的时刻：停着的那一会儿过完、下一行。
    pub fn wake(&self, now: Instant, look: &Look) -> Option<Instant> {
        let (closed, _) = self.walk?;
        let settle = closed + Duration::from_millis(look.settle_ms);
        if now < settle {
            return Some(settle);
        }
        let row = Duration::from_millis(look.row_ms.max(1));
        let rows = now.saturating_duration_since(settle).as_millis() / row.as_millis();
        Some(settle + row * (u32::try_from(rows).unwrap_or(u32::MAX - 1) + 1))
    }
}
