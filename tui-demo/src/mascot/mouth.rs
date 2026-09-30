//! 吉祥物的嘴（蓝图 `tui.md`「空会话的首页」第 9 条，照网页演示）：锯齿线是嘴，张多大是 0 合着到 1 张到最大，照半衰期
//! 缓着变。被列表顶上去张一下、走下来时一路张着；没人动好一阵以后隔一阵打一个哈欠（张到最大、闭着眼）。

use std::time::{Duration, Instant};

use super::model::MouthLook as Look;

/// 嘴：现在张多大，一会儿要张成多大、到什么时候、缓多快，下一次看要不要打哈欠。
#[derive(Debug)]
pub struct Mouth {
    now: f64,
    /// 张一阵：张成多大、到什么时候、缓动的半衰期。
    gaping: Option<(f64, Instant, Duration)>,
    /// 这一刻起一路张着（走下来）：张多大；没有是 `None`。
    holding: Option<f64>,
    /// 正在打哈欠：到这一刻为止（闭着眼）。
    yawn_until: Option<Instant>,
    next_yawn: Option<Instant>,
    at: Option<Instant>,
    seed: u64,
}

impl Mouth {
    /// 合着的，照 `seed` 定打哈欠的间隔（给 0 也行）。
    pub fn new(seed: u64) -> Self {
        Self {
            now: 0.0,
            gaping: None,
            holding: None,
            yawn_until: None,
            next_yawn: None,
            at: None,
            seed: seed | 1,
        }
    }

    /// 张成 `level` 张 `ms` 毫秒再回去，照 `half_ms` 缓着变。
    pub fn gape(&mut self, now: Instant, level: f64, ms: u64, half_ms: u64) {
        let until = now + Duration::from_millis(ms);
        self.gaping = Some((level, until, Duration::from_millis(half_ms)));
    }

    /// 被列表顶上去：张一下。
    pub fn hop(&mut self, now: Instant, look: &Look) {
        self.gape(now, look.hop, look.hop_ms, look.half_life_ms);
    }

    /// 一行一行走下来时一路张着；走完了给 `None`。
    pub fn hold(&mut self, level: Option<f64>) {
        self.holding = level;
    }

    /// 走到 `now` 这一刻，交回张多大。`quiet` 是从什么时候起没人动（有人动着是 `None`）：没人动够
    /// `idle_ms` 的 `yawn_idle` 倍、到了看的时候，打一个哈欠。
    pub fn step(&mut self, now: Instant, look: &Look, quiet: Option<Instant>, idle_ms: u64) -> f64 {
        let due = *self
            .next_yawn
            .get_or_insert_with(|| now + between(&mut self.seed, look.yawn_every_ms));
        if now >= due {
            self.next_yawn = Some(now + between(&mut self.seed, look.yawn_every_ms));
            let long = Duration::from_millis(idle_ms.saturating_mul(look.yawn_idle));
            if quiet.is_some_and(|q| now >= q + long) {
                self.gape(now, look.yawn, look.yawn_ms, look.yawn_half_life_ms);
                self.yawn_until = Some(now + Duration::from_millis(look.yawn_ms));
            }
        }
        if self.gaping.is_some_and(|(_, until, _)| now >= until) {
            self.gaping = None;
        }
        let (want, half) = match (self.gaping, self.holding) {
            (Some((level, _, half)), _) => (level, half),
            (None, Some(level)) => (level, Duration::from_millis(look.half_life_ms)),
            (None, None) => (look.rest, Duration::from_millis(look.half_life_ms)),
        };
        let dt = self
            .at
            .map_or(Duration::ZERO, |at| now.saturating_duration_since(at))
            .min(half.max(Duration::from_millis(1)));
        self.at = Some(now);
        let keep = if half.is_zero() {
            0.0
        } else {
            0.5_f64.powf(dt.as_secs_f64() / half.as_secs_f64())
        };
        let next = want + (self.now - want) * keep;
        self.now = if (next - want).abs() < 0.02 {
            want
        } else {
            next
        };
        self.now
    }

    /// 在打哈欠：闭着眼。
    pub fn yawning(&self, now: Instant) -> bool {
        self.yawn_until.is_some_and(|u| now < u)
    }

    /// 下一次该画的时刻：张着、变着的时候照 `frame_ms` 一帧；停着的等下一次看要不要打哈欠。
    pub fn wake(&self, now: Instant, look: &Look) -> Option<Instant> {
        let moving = self.gaping.is_some()
            || (self.now - self.holding.unwrap_or(look.rest)).abs() > f64::EPSILON;
        if moving {
            return Some(now + Duration::from_millis(look.frame_ms));
        }
        self.next_yawn.filter(|at| *at > now)
    }
}

/// `[最小, 最大]` 毫秒里随机一个时长（xorshift）。
fn between(seed: &mut u64, [low, high]: [u64; 2]) -> Duration {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    let span = high.saturating_sub(low) + 1;
    Duration::from_millis(low.min(high) + *seed % span)
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::Mouth;
    use crate::config::Config;

    #[test]
    fn a_hop_opens_then_closes_again() {
        let look = Config::builtin().unwrap().mascot.mouth;
        let t0 = Instant::now();
        let mut m = Mouth::new(7);
        assert_eq!(m.step(t0, &look, None, 3000), look.rest, "平常合着");
        m.hop(t0, &look);
        let mut open = 0.0_f64;
        for k in 1..=5 {
            open = open.max(m.step(t0 + Duration::from_millis(16 * k), &look, None, 3000));
        }
        assert!(open > 0.3, "被顶上去张开了：{open}");
        let later = t0 + Duration::from_millis(look.hop_ms + 400);
        for k in 0..30 {
            m.step(later + Duration::from_millis(16 * k), &look, None, 3000);
        }
        assert_eq!(
            m.step(later + Duration::from_millis(600), &look, None, 3000),
            look.rest,
            "张完合上"
        );
        assert!(
            m.wake(later + Duration::from_millis(600), &look)
                .is_some_and(|at| at > later),
            "停着只等下一次看哈欠"
        );
    }

    #[test]
    fn a_yawn_comes_only_after_a_long_quiet_and_closes_the_eyes() {
        let look = Config::builtin().unwrap().mascot.mouth;
        let t0 = Instant::now();
        let mut m = Mouth::new(3);
        m.step(t0, &look, None, 3000);
        let check = t0 + Duration::from_millis(look.yawn_every_ms[1] + 1);
        // 刚有人动过：到了看的时候也不打。
        m.step(check, &look, Some(check), 3000);
        assert!(!m.yawning(check));
        // 没人动够久：下一次看的时候打。
        let quiet = t0;
        let later = check + Duration::from_millis(look.yawn_every_ms[1] + 1);
        m.step(later, &look, Some(quiet), 3000);
        assert!(m.yawning(later), "闭着眼打哈欠");
        let mut open = 0.0_f64;
        for k in 1..=60 {
            open = open.max(m.step(
                later + Duration::from_millis(16 * k),
                &look,
                Some(quiet),
                3000,
            ));
        }
        assert!(open > 0.9, "张到最大：{open}");
        assert!(!m.yawning(later + Duration::from_millis(look.yawn_ms + 1)));
    }

    #[test]
    fn walking_down_keeps_it_open() {
        let look = Config::builtin().unwrap().mascot.mouth;
        let t0 = Instant::now();
        let mut m = Mouth::new(5);
        m.hold(Some(look.walk));
        for k in 0..40 {
            m.step(t0 + Duration::from_millis(16 * k), &look, None, 3000);
        }
        assert!(
            (m.step(t0 + Duration::from_millis(700), &look, None, 3000) - look.walk).abs() < 0.03
        );
        m.hold(None);
        for k in 0..40 {
            m.step(t0 + Duration::from_millis(700 + 16 * k), &look, None, 3000);
        }
        assert_eq!(
            m.step(t0 + Duration::from_millis(1400), &look, None, 3000),
            look.rest
        );
    }
}
