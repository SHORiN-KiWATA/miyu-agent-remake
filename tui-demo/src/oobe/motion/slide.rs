//! 换一步的滑动（「第一次打开的引导」第 10 条）：原来那块往左滑走、下一块从右边滑进来；回上一步反过来。先快后慢。

use std::time::{Duration, Instant};

/// 正在滑：从哪一刻起、是不是回上一步。
#[derive(Debug, Clone, Copy)]
pub struct Slide {
    start: Instant,
    /// 回上一步：原来那块往右走、上一块从左边进来。
    pub back: bool,
}

impl Slide {
    /// 从 `now` 起滑。
    pub fn new(now: Instant, back: bool) -> Self {
        Self { start: now, back }
    }

    /// 这一刻两块各往右挪几列（往左是负的）：`(原来那块, 新的那块)`；滑完了是 `None`。
    pub fn offsets(&self, now: Instant, total: Duration, width: u16) -> Option<(i32, i32)> {
        let p = super::fraction(self.start, now, total);
        if p >= 1.0 {
            return None;
        }
        let moved = (super::ease_out(p) * f64::from(width)).round() as i32;
        let width = i32::from(width);
        Some(if self.back {
            (moved, moved - width)
        } else {
            (-moved, width - moved)
        })
    }
}
