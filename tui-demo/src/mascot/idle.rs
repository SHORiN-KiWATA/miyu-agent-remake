//! 吉祥物的待机小动作（蓝图 `tui.md`「空会话的首页」第 8 条）：一直隔一阵眨一次眼（长短、间隔随机，
//! 有时连眨两下）；没人动时一阵一阵地摇头（角度随机、停一阵再换，有时回正前方）、隔一阵抖一下耳朵，一动就停。

use std::f64::consts::PI;
use std::time::{Duration, Instant};

use super::model::Idle as Look;

/// 这一刻的小动作。
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Idling {
    /// 闭着眼。
    pub blink: bool,
    /// 耳朵往外多歪多少（弧度）。
    pub ear: f64,
    /// 没人动：要摇到的角度 `(yaw, pitch)`（度）；有人动是 `None`。
    pub glance: Option<(f64, f64)>,
}

/// 小动作的时间表。
#[derive(Debug)]
pub struct Idle {
    /// 最后有人动的时刻；还没画过是 `None`。
    active: Option<Instant>,
    next_blink: Option<Instant>,
    /// 正闭着眼：到这一刻睁开。
    blink_until: Option<Instant>,
    /// 下一次眨完接着再眨一下。
    double: bool,
    glance: Option<(f64, f64)>,
    next_glance: Option<Instant>,
    next_twitch: Option<Instant>,
    /// 正在抖：起止和这一次的幅度。
    twitch: Option<(Instant, Instant, f64)>,
    seed: u64,
}

impl Idle {
    /// 空的，照 `seed` 定随机的间隔（给 0 也行）。
    pub fn new(seed: u64) -> Self {
        Self {
            active: None,
            next_blink: None,
            blink_until: None,
            double: false,
            glance: None,
            next_glance: None,
            next_twitch: None,
            twitch: None,
            seed: seed | 1,
        }
    }

    /// 有人按键、动鼠标了：停下摇头和抖耳朵。
    pub fn poke(&mut self, now: Instant) {
        self.active = Some(now);
        self.glance = None;
        self.next_glance = None;
        self.next_twitch = None;
        self.twitch = None;
    }

    /// 最后有人动的时刻（嘴照它看要不要打哈欠）；还没画过是 `None`。
    pub fn quiet_since(&self) -> Option<Instant> {
        self.active
    }

    /// 没人动了。
    pub fn idling(&self, now: Instant, look: &Look) -> bool {
        self.active
            .is_some_and(|a| now >= a + Duration::from_millis(look.after_ms))
    }

    /// 走到 `now` 这一刻，给出这一刻的样子。
    pub fn pose(&mut self, now: Instant, look: &Look) -> Idling {
        self.active.get_or_insert(now);
        let blink = self.blink(now, look);
        if !self.idling(now, look) {
            return Idling {
                blink,
                ..Idling::default()
            };
        }
        Idling {
            blink,
            ear: self.twitch(now, look),
            glance: Some(self.glance(now, look)),
        }
    }

    /// 眨眼：到点闭上，闭够了睁开；连眨的隔一小会儿再眨一下，不然隔一阵。
    fn blink(&mut self, now: Instant, look: &Look) -> bool {
        if self.next_blink.is_none() && self.blink_until.is_none() {
            self.next_blink = Some(now + self.between(look.blink_every_ms));
        }
        if let Some(until) = self.blink_until.filter(|u| *u <= now) {
            self.blink_until = None;
            let gap = if std::mem::take(&mut self.double) {
                Duration::from_millis(look.double_gap_ms)
            } else {
                self.double = self.chance() < look.double_blink;
                self.between(look.blink_every_ms)
            };
            self.next_blink = Some(until + gap);
        }
        if let Some(at) = self.next_blink.filter(|at| *at <= now) {
            self.next_blink = None;
            self.blink_until = Some(at + self.between(look.blink_ms));
        }
        self.blink_until.is_some_and(|u| u > now)
    }

    /// 摇头：到点换一个角度，停一阵；有时回正前方。
    fn glance(&mut self, now: Instant, look: &Look) -> (f64, f64) {
        if self.next_glance.is_none_or(|at| at <= now) {
            let home = self.chance() < look.glance_home && self.glance.is_some();
            let next = if home {
                (0.0, 0.0)
            } else {
                let yaw = (self.chance() * 2.0 - 1.0) * look.glance_yaw;
                let pitch = (self.chance() * 2.0 - 1.0) * look.glance_pitch;
                (yaw, pitch)
            };
            self.glance = Some(next);
            self.next_glance = Some(now + self.between(look.glance_hold_ms));
        }
        self.glance.unwrap_or_default()
    }

    /// 抖耳朵：到点抖一下，幅度随机，抖完再定下一次。
    fn twitch(&mut self, now: Instant, look: &Look) -> f64 {
        if self.next_twitch.is_none() && self.twitch.is_none() {
            self.next_twitch = Some(now + self.between(look.twitch_every_ms));
        }
        if let Some(at) = self.next_twitch.filter(|at| *at <= now) {
            let [low, high] = look.twitch_tilt;
            let size = low + (high - low) * self.chance();
            self.twitch = Some((at, at + Duration::from_millis(look.twitch_ms), size));
            self.next_twitch = None;
        }
        let Some((start, end, size)) = self.twitch else {
            return 0.0;
        };
        if now >= end {
            self.twitch = None;
            self.next_twitch = Some(end + self.between(look.twitch_every_ms));
            return 0.0;
        }
        let span = end.saturating_duration_since(start).as_secs_f64().max(1e-3);
        size * (PI * now.saturating_duration_since(start).as_secs_f64() / span).sin()
    }

    /// 下一次该画的时刻：抖耳朵时一小段一帧；别的时候等到下一次眨眼、睁眼、换角度、抖耳朵或进入待机。
    /// 摇过去的那一会儿由转头的缓动照它自己的节拍画。
    pub fn wake(&self, now: Instant, look: &Look) -> Option<Instant> {
        let since = self.active? + Duration::from_millis(look.after_ms);
        if self.twitch.is_some() {
            return Some(now + Duration::from_millis(look.frame_ms));
        }
        let idle = self.idling(now, look);
        [
            self.next_blink,
            self.blink_until,
            Some(since),
            idle.then_some(self.next_glance).flatten(),
            idle.then_some(self.next_twitch).flatten(),
        ]
        .into_iter()
        .flatten()
        .filter(|at| *at > now)
        .min()
    }

    /// 0–1 之间一个随机数。
    fn chance(&mut self) -> f64 {
        (self.next() % 1_000_000) as f64 / 1_000_000.0
    }

    /// `[最小, 最大]` 毫秒里随机一个时长。
    fn between(&mut self, [low, high]: [u64; 2]) -> Duration {
        let span = high.saturating_sub(low) + 1;
        Duration::from_millis(low.min(high) + self.next() % span)
    }

    /// xorshift：够用，不为这个引随机数库。
    fn next(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
}
