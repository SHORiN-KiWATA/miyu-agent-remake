//! 好了那一屏（「第一次打开的引导」第 11 条）：吉祥物从左边走回中间（一路张着嘴），原地转一圈，「好了」一个字一个字
//! 打出来，再露出那一行三样和「回车开始聊天」。

use std::time::{Duration, Instant};

use super::{ease_out, fraction, swing};
use crate::oobe::look::Look;

/// 好了那一屏的一帧。
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Finale {
    /// 从左边走到中间走了几成（0 到 1）。
    pub walk: f64,
    /// 还差多少度转完一圈（负的，从 -360 到 0）。
    pub spin: f64,
    /// 嘴张多大。
    pub mouth: f64,
    /// 标题打出来几个字。
    pub typed: usize,
    /// 下面两行淡出来几成。
    pub sub: f64,
    /// 播完了。
    pub ready: bool,
}

/// 从 `start` 起，标题有 `chars` 个字，`now` 这一刻的样子。
pub fn finale(start: Instant, now: Instant, look: &Look, chars: usize) -> Finale {
    let ms = Duration::from_millis;
    let walked = start + ms(look.done.walk_ms);
    let spun = walked + ms(look.done.spin_ms);
    let typed_end = spun + ms(look.intro.type_ms) * u32::try_from(chars).unwrap_or(u32::MAX);
    let walk = ease_out(fraction(start, now, ms(look.done.walk_ms)));
    let spin = now.checked_duration_since(walked).map_or(0.0, |_| {
        swing(
            fraction(walked, now, ms(look.done.spin_ms)),
            look.intro.spin_swing,
        )
    });
    let typed = now.checked_duration_since(spun).map_or(0, |d| {
        let n = d.as_millis() / u128::from(look.intro.type_ms.max(1)) + 1;
        usize::try_from(n).unwrap_or(usize::MAX).min(chars)
    });
    let sub = now
        .checked_duration_since(typed_end)
        .map_or(0.0, |_| fraction(typed_end, now, ms(look.intro.sub_ms)));
    let walking = now < walked;
    Finale {
        walk,
        spin: if now < walked {
            0.0
        } else {
            -360.0 * (1.0 - spin)
        },
        mouth: if walking { look.react.hop_mouth } else { 0.0 },
        typed,
        sub,
        ready: now >= typed_end + ms(look.intro.sub_ms),
    }
}
