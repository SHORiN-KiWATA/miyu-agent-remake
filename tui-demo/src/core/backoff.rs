//! 连不上时隔多久再试（蓝图 `tui.md`「连核心」第 7 条）：从 `layout.json` 的 `reconnect_ms` 第一个数起，每次翻倍，
//! 最多第二个数；连上一次以后从头算。

use std::time::Duration;

/// 重连的间隔。
#[derive(Debug, Clone)]
pub struct Backoff {
    first: u64,
    most: u64,
    next: u64,
}

impl Backoff {
    /// `[最短, 最长]`，毫秒。
    pub fn new([first, most]: [u64; 2]) -> Self {
        let first = first.max(1);
        Self {
            first,
            most: most.max(first),
            next: first,
        }
    }

    /// 这一次等多久；下一次翻倍。
    pub fn next(&mut self) -> Duration {
        let now = self.next;
        self.next = now.saturating_mul(2).min(self.most);
        Duration::from_millis(now)
    }

    /// 连上了：下次断了从最短的起。
    pub fn reset(&mut self) {
        self.next = self.first;
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::Backoff;

    #[test]
    fn it_doubles_up_to_the_most_and_starts_over_after_a_reset() {
        let mut wait = Backoff::new([500, 5000]);
        let got: Vec<Duration> = (0..6).map(|_| wait.next()).collect();
        let ms = |v: &[u64]| {
            v.iter()
                .map(|&m| Duration::from_millis(m))
                .collect::<Vec<_>>()
        };
        assert_eq!(got, ms(&[500, 1000, 2000, 4000, 5000, 5000]));
        wait.reset();
        assert_eq!(wait.next(), Duration::from_millis(500));
    }
}
