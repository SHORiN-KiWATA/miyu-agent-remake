//! 星点（「第一次打开的引导」第 8、11 条）：开场、好了两屏从四周冒出来往中间聚，越近越亮、到了就灭；聚完留几颗在四周
//! 一闪一闪；走的时候往外散开。位置是窗口里的几成（0 到 1），画的时候再换成格子。

use std::f64::consts::TAU;
use std::time::{Duration, Instant};

use super::{ease_in, ease_out, fraction};
use crate::oobe::look::Stars as Look;
use crate::rng::Rng;

/// 一颗星这一帧：在哪（窗口宽、高的几成）、多亮（0 到 1）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Star {
    /// 横着在几成。
    pub x: f64,
    /// 竖着在几成。
    pub y: f64,
    /// 多亮。
    pub level: f64,
}

/// 一群星：种子定每一颗从哪来、晚多久出发、闪多快。
#[derive(Debug, Clone)]
pub struct Stars {
    seed: u64,
    /// 闪的钟从这一刻算。
    born: Instant,
    gather: Option<Instant>,
    scatter: Option<Instant>,
}

/// 种子定下来的一颗：从哪个方向来、多远、晚多久出发（`lead_ms` 的几成）、闪多快、从哪一相起闪。
struct Seeded {
    angle: f64,
    reach: f64,
    lead: f64,
    period_ms: u64,
    phase: f64,
}

/// 留下来闪的那几颗离中间至少、至多多远（窗口的几成）：不压在吉祥物身上。
const LINGER_REACH: [f64; 2] = [0.34, 0.62];
/// 聚的那几颗从多远来。
const GATHER_REACH: [f64; 2] = [0.45, 0.75];
/// 闪的时候最暗是几成。
const DIMMEST: f64 = 0.15;

impl Stars {
    /// 从 `now` 起往中间聚。
    pub fn gather(now: Instant, seed: u64) -> Self {
        Self {
            seed,
            born: now,
            gather: Some(now),
            scatter: None,
        }
    }

    /// 只有四周闪着的那几颗（回到欢迎页时）。
    pub fn quiet(now: Instant, seed: u64) -> Self {
        Self {
            seed,
            born: now,
            gather: None,
            scatter: None,
        }
    }

    /// 从 `now` 起往外散开。
    pub fn scatter(&mut self, now: Instant) {
        self.scatter = Some(now);
    }

    /// 这一刻的星；`center` 是吉祥物在窗口里的几成。
    pub fn at(&self, now: Instant, look: &Look, center: (f64, f64)) -> Vec<Star> {
        let seeds = self.seeds(look);
        let ms = Duration::from_millis;
        if let Some(start) = self.scatter {
            let p = fraction(start, now, ms(look.scatter_ms));
            if p >= 1.0 {
                return Vec::new();
            }
            let out = ease_out(p) * 1.4;
            return seeds
                .iter()
                .take(look.count.max(look.linger))
                .filter_map(|s| place(center, s.angle, s.reach * out, 1.0 - p))
                .collect();
        }
        if let Some(start) = self.gather.filter(|g| now < *g + ms(look.gather_ms)) {
            let t = now.saturating_duration_since(start).as_secs_f64() * 1000.0;
            return seeds
                .iter()
                .take(look.count)
                .filter_map(|s| {
                    let delay = s.lead * look.lead_ms as f64;
                    let travel = (look.gather_ms as f64 - delay).max(1.0);
                    let p = ((t - delay) / travel).clamp(0.0, 1.0);
                    (p < 1.0).then_some(())?;
                    let reach = s.reach.min(room(center, s.angle) * 0.98) * (1.0 - ease_in(p));
                    place(center, s.angle, reach, 0.3 + 0.7 * p)
                })
                .collect();
        }
        let t = now.saturating_duration_since(self.born).as_secs_f64() * 1000.0;
        seeds
            .iter()
            .take(look.linger)
            .filter_map(|s| {
                let wave = 0.5 + 0.5 * (TAU * t / s.period_ms.max(1) as f64 + s.phase).sin();
                let reach = (LINGER_REACH[0] + (LINGER_REACH[1] - LINGER_REACH[0]) * s.lead)
                    .min(room(center, s.angle) * 0.98);
                place(center, s.angle, reach, DIMMEST + (1.0 - DIMMEST) * wave)
            })
            .collect()
    }

    /// 在聚、在散：照动着的节拍画；只剩一闪一闪的照 `frame_ms`。
    pub fn moving(&self, now: Instant, look: &Look) -> bool {
        let ms = Duration::from_millis;
        self.gather.is_some_and(|g| now < g + ms(look.gather_ms))
            || self.scatter.is_some_and(|s| now < s + ms(look.scatter_ms))
    }

    /// 种子定下来的每一颗：同一个种子一样。
    fn seeds(&self, look: &Look) -> Vec<Seeded> {
        let mut rng = Rng::new(self.seed);
        let mut unit = move || (rng.next() % 10_000) as f64 / 10_000.0;
        (0..look.count.max(look.linger))
            .map(|_| Seeded {
                angle: unit() * TAU,
                reach: GATHER_REACH[0] + (GATHER_REACH[1] - GATHER_REACH[0]) * unit(),
                lead: unit(),
                period_ms: look.twinkle_ms[0]
                    + ((look.twinkle_ms[1].saturating_sub(look.twinkle_ms[0])) as f64 * unit())
                        as u64,
                phase: unit() * TAU,
            })
            .collect()
    }
}

/// 从 `center` 朝 `angle` 走 `reach`；出了窗口的不画（散开时飞出去就没了，不贴着边堆成一条线）。
fn place(center: (f64, f64), angle: f64, reach: f64, level: f64) -> Option<Star> {
    let x = center.0 + angle.cos() * reach;
    let y = center.1 + angle.sin() * reach;
    ((0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y)).then(|| Star {
        x,
        y,
        level: level.clamp(0.0, 1.0),
    })
}

/// 从 `center` 朝 `angle` 最多走多远还在窗口里。
fn room(center: (f64, f64), angle: f64) -> f64 {
    let along = |c: f64, d: f64| {
        if d > 1e-9 {
            (1.0 - c) / d
        } else if d < -1e-9 {
            c / -d
        } else {
            f64::INFINITY
        }
    };
    along(center.0, angle.cos()).min(along(center.1, angle.sin()))
}
