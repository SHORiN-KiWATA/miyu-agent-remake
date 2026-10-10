//! 欢迎页的开场（「第一次打开的引导」第 8 条）：转一整圈（先蓄力、再加速、冲过头回正）、从黑到亮；落定以后抖一下耳朵、连眨两下眼；字一个一个打出来、
//! 打字时嘴一张一合；说明淡出来；最后露出「回车开始」。按什么键都先跳到最后一帧。

use std::time::{Duration, Instant};

use super::{ease_out, fraction, swing};
use crate::oobe::look::Intro as Look;

/// 开场：从哪一刻起、跳过了没有。
#[derive(Debug, Clone)]
pub struct Intro {
    start: Instant,
    skipped: bool,
}

/// 开场的一帧。
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Scene {
    /// 整个身子还差多少度转到正面（负的，从 -360 到 0）。
    pub spin: f64,
    /// 暗多少：1 全黑，0 照常。
    pub dark: f64,
    /// 耳朵往外多歪多少（弧度）。
    pub ear: f64,
    /// 闭着眼。
    pub blink: bool,
    /// 嘴张多大。
    pub mouth: f64,
    /// 标题打出来几个字。
    pub typed: usize,
    /// 说明淡出来几成，0 到 1。
    pub sub: f64,
    /// 「回车开始」露出来了：开场完了。
    pub ready: bool,
}

impl Intro {
    /// 从 `now` 起。
    pub fn new(now: Instant) -> Self {
        Self {
            start: now,
            skipped: false,
        }
    }

    /// 跳到最后一帧。
    pub fn skip(&mut self) {
        self.skipped = true;
    }

    /// 标题有 `chars` 个字时，这一刻的样子。
    pub fn scene(&self, now: Instant, look: &Look, chars: usize) -> Scene {
        let done = Scene {
            typed: chars,
            sub: 1.0,
            ready: true,
            ..Scene::default()
        };
        if self.skipped {
            return done;
        }
        let ms = Duration::from_millis;
        let spin = swing(fraction(self.start, now, ms(look.spin_ms)), look.spin_swing);
        let light = ease_out(fraction(self.start, now, ms(look.fade_ms)));
        let landed = self.start + ms(look.spin_ms.max(look.fade_ms));
        let typing = landed + ms(look.settle_ms);
        let typed_end = typing + ms(look.type_ms) * u32::try_from(chars).unwrap_or(u32::MAX);
        if now >= typed_end + ms(look.sub_ms) {
            return done;
        }
        // 落定以后：先抖耳朵，再连眨两下。
        let since = now.checked_duration_since(landed);
        let ear = since.filter(|d| *d < ms(look.twitch_ms)).map_or(0.0, |d| {
            let p = d.as_secs_f64() / ms(look.twitch_ms).as_secs_f64();
            look.twitch_tilt * (p * std::f64::consts::PI).sin()
        });
        let blink = since.is_some_and(|d| {
            let first = ms(look.twitch_ms);
            let second = first + ms(look.blink_ms + look.blink_gap_ms);
            (d >= first && d < first + ms(look.blink_ms))
                || (d >= second && d < second + ms(look.blink_ms))
        });
        // 打字：一个字一拍，单数的字张嘴、双数的合上，像一个字一个字地说。
        let into = now.checked_duration_since(typing);
        let typed = into.map_or(0, |d| {
            let n = d.as_millis() / u128::from(look.type_ms.max(1)) + 1;
            usize::try_from(n).unwrap_or(usize::MAX).min(chars)
        });
        let talking = into.is_some() && now < typed_end && typed % 2 == 1;
        let sub = now
            .checked_duration_since(typed_end)
            .map_or(0.0, |_| fraction(typed_end, now, ms(look.sub_ms)));
        Scene {
            spin: -360.0 * (1.0 - spin),
            dark: 1.0 - light,
            ear,
            blink,
            mouth: if talking { look.talk } else { 0.0 },
            typed,
            sub,
            ready: false,
        }
    }

    /// 开场完了没有。
    pub fn finished(&self, now: Instant, look: &Look, chars: usize) -> bool {
        self.scene(now, look, chars).ready
    }
}
