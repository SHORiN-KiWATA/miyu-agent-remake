//! 引导里动的东西（蓝图 `tui.md`「第一次打开的引导」第 8–12、29 条）：都是「给一个时刻，算出这一帧」，不存钟、不起线程，
//! 测试拿假钟一帧帧查。开场的时间线在 `intro.rs`，好了那一屏的在 `finale.rs`，星点在 `stars.rs`，换一步的滑动在 `slide.rs`，吉祥物的反应在
//! `react.rs`。

mod finale;
mod intro;
mod react;
mod slide;
mod stars;

pub use finale::{Finale, finale};
pub use intro::Intro;
pub use react::React;
pub use slide::Slide;
pub use stars::{Star, Stars};

use std::time::{Duration, Instant};

/// 从 `start` 到 `now` 走了 `total` 的几成，0 到 1；`total` 是 0 的算走完。
pub fn fraction(start: Instant, now: Instant, total: Duration) -> f64 {
    if total.is_zero() {
        return 1.0;
    }
    (now.saturating_duration_since(start).as_secs_f64() / total.as_secs_f64()).clamp(0.0, 1.0)
}

/// 先快后慢。
pub fn ease_out(p: f64) -> f64 {
    let p = p.clamp(0.0, 1.0);
    1.0 - (1.0 - p).powi(3)
}

/// 先慢后快。
/// 转圈的曲线（「第一次打开的引导」第 8 条，2026-10-09 项目主人：「旋转的动画可以有一个曲线」）：先往反方向蓄一下力，
/// 再加速，最后冲过头一点再回正（ease-in-out-back）。`c` 是蓄、冲的力度，0 是平常的先慢、再快、再慢。
pub fn swing(p: f64, c: f64) -> f64 {
    let p = p.clamp(0.0, 1.0);
    let c = c * 1.525;
    let q = 2.0 * p;
    if p < 0.5 {
        q * q * ((c + 1.0) * q - c) / 2.0
    } else {
        let r = q - 2.0;
        (r * r * ((c + 1.0) * r + c) + 2.0) / 2.0
    }
}

pub fn ease_in(p: f64) -> f64 {
    let p = p.clamp(0.0, 1.0);
    p * p
}

#[cfg(test)]
mod tests;
