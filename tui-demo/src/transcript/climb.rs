//! 压缩那一行进度条亮几格（蓝图 `tui.md`「正文」第 9 条）：按整格一顿一顿地追真实的字数。真实的字数够多亮
//! 几格了，随机停一会儿，再一下多亮一到几格；永远不超过真实的字数够的格数，没压好之前封顶。压好了，从当时亮到的
//! 格子一格格走满，停一下再换成结果。

use std::time::{Duration, Instant};

use crate::config::CompactionMotion;
use crate::rng::Rng;

/// 正在压缩的那一行的进度。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Progress {
    /// 核心推来的，已经写了多少字（真的）。
    pub written: u64,
    /// 核心估的要写多少字；以前的核心不给。
    pub expected: Option<u64>,
    /// 进度条亮着几格：一顿一顿地追真实的字数够的格数。
    pub lit: usize,
    /// 这一次压缩从哪一刻开始：过一会儿开始呼吸照它算。
    pub since: Instant,
    /// 下一次追的时刻；没在等的是 `None`。
    next: Option<Instant>,
    /// 压好了，正在走满；走满、停一下以后换成它记着的结果。
    pub finish: Option<Finish>,
}

/// 压好了、正在走满的进度条：走满以后换成的那一行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finish {
    /// 哪一刻压好的。
    pub at: Instant,
    /// 那时亮到第几格。
    pub from: usize,
    /// 换成的那一行的字。
    pub text: String,
    /// 那一行前面绿色的记号。
    pub mark: String,
}

impl Progress {
    /// 刚收到第一条进度。
    pub fn new(written: u64, expected: Option<u64>, now: Instant) -> Self {
        Self {
            written,
            expected,
            lit: 0,
            since: now,
            next: None,
            finish: None,
        }
    }

    /// 压好了：从现在亮到的格子起走满，走满、停一下以后换成 `text`（前面是 `mark`）。
    pub fn done(&mut self, now: Instant, text: String, mark: String) {
        self.finish = Some(Finish {
            at: now,
            from: self.lit,
            text,
            mark,
        });
    }

    /// 走满了、也停够了：该换成结果了。
    pub fn filled(&self, now: Instant, width: usize, look: &CompactionMotion) -> bool {
        let Some(finish) = &self.finish else {
            return false;
        };
        let hold = Duration::from_millis(look.finish_ms + look.finish_hold_ms);
        self.lit >= width && now >= finish.at + hold
    }

    /// 真实的字数，没压好之前封顶 `cap_percent`；核心没给估计的是 `None`。
    pub fn capped(&self, look: &CompactionMotion) -> Option<(u64, u64)> {
        let expected = self.expected?.max(1);
        Some((
            self.written.min(expected * look.cap_percent / 100),
            expected,
        ))
    }

    /// 到点了追一下（每一帧叫一次）。`width` 是条有几格。
    pub fn climb(&mut self, now: Instant, width: usize, look: &CompactionMotion, rng: &mut Rng) {
        // 压好了：照过去的时间一格格走满，不再随机。
        if let Some(finish) = &self.finish {
            let spent = now.saturating_duration_since(finish.at).as_millis();
            let total = u128::from(look.finish_ms.max(1));
            let rest = width.saturating_sub(finish.from) as u128;
            let add = usize::try_from((rest * spent.min(total)).div_ceil(total)).unwrap_or(width);
            self.lit = self.lit.max(finish.from + add).min(width);
            return;
        }
        let Some((written, expected)) = self.capped(look) else {
            return;
        };
        // 真实的字数够亮几整格。
        let cell = expected.div_ceil(width.max(1) as u64).max(1);
        let target = usize::try_from(written / cell).unwrap_or(width).min(width);
        if self.lit >= target {
            self.next = None;
            return;
        }
        match self.next {
            None => {
                let pause = rng.between(look.pause_ms);
                self.next = Some(now + Duration::from_millis(pause));
            }
            Some(at) if now >= at => {
                let step = usize::try_from(rng.between(look.step_cells)).unwrap_or(1);
                self.lit = (self.lit + step.max(1)).min(target);
                self.next = None;
            }
            Some(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::Progress;
    use crate::config::Config;
    use crate::rng::Rng;

    #[test]
    fn the_bar_catches_up_in_whole_cell_bursts_and_never_passes_the_real_count() {
        let look = Config::builtin().unwrap().layout.compaction;
        let mut rng = Rng::new(42);
        let t0 = Instant::now();
        // 估 20,000 字、20 格：一格 1,000 字。
        let mut p = Progress::new(900, Some(20_000), t0);
        p.climb(t0, 20, &look, &mut rng);
        assert_eq!(p.lit, 0, "不够一整格：不亮");
        // 真实的写到 6,500 字，够亮 6 格：先停一顿，不当场追。
        p.written = 6_500;
        p.climb(t0, 20, &look, &mut rng);
        assert_eq!(p.lit, 0, "先停一顿");
        let too_soon = t0 + Duration::from_millis(look.pause_ms[0] - 1);
        p.climb(too_soon, 20, &look, &mut rng);
        assert_eq!(p.lit, 0, "最短的一顿还没到");
        // 等够最长的一顿：一下多亮 1 到 3 格。
        let late = t0 + Duration::from_millis(look.pause_ms[1]);
        p.climb(late, 20, &look, &mut rng);
        assert!((1..=3).contains(&p.lit), "一下多亮 1 到 3 格：{}", p.lit);
        // 一直追下去：追到真实的够的 6 格为止，不超过。
        let mut now = late;
        for _ in 0..100 {
            now += Duration::from_millis(look.pause_ms[1]);
            p.climb(now, 20, &look, &mut rng);
        }
        assert_eq!(p.lit, 6, "追到真实的够的格数就停");
    }

    #[test]
    fn it_stops_below_the_cap_until_done_and_needs_an_estimate() {
        let look = Config::builtin().unwrap().layout.compaction;
        let mut rng = Rng::new(3);
        let t0 = Instant::now();
        let mut p = Progress::new(30_000, Some(20_000), t0);
        let mut now = t0;
        for _ in 0..100 {
            now += Duration::from_millis(look.pause_ms[1]);
            p.climb(now, 20, &look, &mut rng);
        }
        let capped = 20 * usize::try_from(look.cap_percent).unwrap() / 100;
        assert_eq!(p.lit, capped, "写过了估的：封顶");
        assert_eq!(p.capped(&look), Some((19_000, 20_000)));
        let mut blind = Progress::new(5_000, None, t0);
        blind.climb(now, 20, &look, &mut rng);
        assert_eq!(
            (blind.lit, blind.capped(&look)),
            (0, None),
            "核心没给估计：不画条，也不追"
        );
    }
}
