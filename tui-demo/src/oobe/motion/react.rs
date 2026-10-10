//! 吉祥物在引导里的反应（「第一次打开的引导」第 10 条）：换一步跳一下、耳朵扑一下；试连接时抬头等；连上了跳一下、连眨
//! 两下；没连上耷拉耳朵、摇两下头，耳朵停到下一次按键再竖回来。打字时歪着头看、耳朵跟着抖（2026-10-09 项目主人：
//! 「灵动性也不够，略显死板」）。换选项不点头（2026-10-10 项目主人：「每次都会点头，很鬼畜」）。

use std::f64::consts::{PI, TAU};
use std::time::{Duration, Instant};

use crate::oobe::look::React as Look;

/// 正在做的反应。
#[derive(Debug, Clone, Default)]
pub struct React {
    hop: Option<Instant>,
    shake: Option<Instant>,
    blinks: Option<Instant>,
    /// 这一阵打字从什么时候起、最后打的一个字在什么时候。
    typing: Option<(Instant, Instant)>,
    /// 耳朵耷拉着。
    pub droop: bool,
    /// 在等（试连接）：抬头。
    pub waiting: bool,
}

/// 这一帧反应叠上去的样子。
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Act {
    /// 往上提几行。
    pub lift: u16,
    /// 左右多转多少度。
    pub yaw: f64,
    /// 上下看哪（抬头时有），盖过转头。
    pub pitch: Option<f64>,
    /// 耳朵往外多歪多少（弧度）。
    pub ear: f64,
    /// 嘴张多大。
    pub mouth: f64,
    /// 闭着眼。
    pub blink: bool,
    /// 歪头多少度。
    pub roll: f64,
}

impl React {
    /// 跳一下（换一步）。
    pub fn hop(&mut self, now: Instant) {
        self.hop = Some(now);
    }

    /// 高兴：跳一下、连眨两下，耳朵竖回来（连上了）。
    pub fn happy(&mut self, now: Instant) {
        self.hop = Some(now);
        self.blinks = Some(now);
        self.shake = None;
        self.droop = false;
        self.waiting = false;
    }

    /// 失望：耷拉耳朵、摇两下头（没连上）。
    pub fn sad(&mut self, now: Instant) {
        self.shake = Some(now);
        self.droop = true;
        self.waiting = false;
    }

    /// 按了键：耳朵竖回来。
    pub fn poke(&mut self) {
        self.droop = false;
    }

    /// 打了一个字：歪着头看，耳朵抖一下。停手一阵（`tilt_hold_ms`）再回正；上一阵还没回正的接着歪着。
    pub fn typed(&mut self, now: Instant, look: &Look) {
        let ms = Duration::from_millis;
        let start = match self.typing {
            Some((start, last)) if now < last + ms(look.tilt_hold_ms + look.tilt_ease_ms) => start,
            _ => now,
        };
        self.typing = Some((start, now));
    }

    /// 这一刻叠上去的样子。
    pub fn act(&self, now: Instant, look: &Look) -> Act {
        let ms = Duration::from_millis;
        let mut act = Act {
            ear: if self.droop { look.droop } else { 0.0 },
            pitch: self.waiting.then_some(look.look_up),
            ..Act::default()
        };
        if let Some(p) = self.hop.and_then(|at| running(at, now, ms(look.hop_ms))) {
            let up = (p * PI).sin();
            act.lift = (up * f64::from(look.hop_rows)).round() as u16;
            act.mouth = look.hop_mouth * up;
            act.ear += look.hop_ear * up;
        }
        if let Some((start, last)) = self.typing {
            act.roll = look.tilt * tilt(start, last, now, look);
            if let Some(p) = running(last, now, ms(look.type_ear_ms)) {
                act.ear += look.type_ear * (p * PI).sin();
            }
        }
        if let Some(p) = self
            .shake
            .and_then(|at| running(at, now, ms(look.shake_ms)))
        {
            act.yaw = look.shake_yaw * (p * TAU * f64::from(look.shakes)).sin() * (1.0 - p);
        }
        if let Some(at) = self.blinks {
            let d = now.saturating_duration_since(at);
            let second = ms(look.blink_ms + look.blink_gap_ms);
            act.blink = d < ms(look.blink_ms) || (d >= second && d < second + ms(look.blink_ms));
        }
        act
    }

    /// 还在动（要接着画）。
    pub fn busy(&self, now: Instant, look: &Look) -> bool {
        let ms = Duration::from_millis;
        let blinks = ms(look.blink_ms * 2 + look.blink_gap_ms);
        let typing = look.tilt_hold_ms + look.tilt_ease_ms;
        self.typing
            .is_some_and(|(_, last)| now < last + ms(typing).max(ms(look.type_ear_ms)))
            || self
                .hop
                .is_some_and(|at| running(at, now, ms(look.hop_ms)).is_some())
            || self
                .shake
                .is_some_and(|at| running(at, now, ms(look.shake_ms)).is_some())
            || self
                .blinks
                .is_some_and(|at| running(at, now, blinks).is_some())
    }
}

/// 歪头歪到几成：从 `start` 起慢慢歪过去，最后一个字（`last`）以后再歪着留一阵，然后慢慢回正。
fn tilt(start: Instant, last: Instant, now: Instant, look: &Look) -> f64 {
    let ms = Duration::from_millis;
    let ease = ms(look.tilt_ease_ms.max(1));
    let into = super::ease_out(super::fraction(start, now, ease));
    let back = last + ms(look.tilt_hold_ms);
    let out = super::fraction(back, now, ease);
    into * (1.0 - super::ease_out(out))
}

/// 从 `at` 起、`total` 那么长的一段，`now` 走到几成；没开始、走完了是 `None`。
fn running(at: Instant, now: Instant, total: Duration) -> Option<f64> {
    let d = now.checked_duration_since(at)?;
    (d < total).then(|| d.as_secs_f64() / total.as_secs_f64())
}
