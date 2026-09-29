//! 界面上要的一点随机（压缩那一行进度条一顿一顿地追，蓝图 `tui.md`「正文」第 9 条）：xorshift，够用，不为这个
//! 引随机数库。提示、运行状态行的词、吉祥物的小动作各有一份一样的，以后可以换成它。

use std::time::{SystemTime, UNIX_EPOCH};

/// 随机数。
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    /// 照给的种子起；0 换成 1（xorshift 从 0 起永远是 0）。
    pub fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    /// 照现在的时刻起。
    pub fn from_clock() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(1, |d| d.as_nanos() as u64);
        Self::new(nanos)
    }

    /// 下一个。
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// `[最小, 最大]` 里随机一个，两头都算。
    pub fn between(&mut self, [low, high]: [u64; 2]) -> u64 {
        let (low, high) = (low.min(high), low.max(high));
        low + self.next() % (high - low + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::Rng;

    #[test]
    fn between_stays_inside_both_ends() {
        let mut rng = Rng::new(7);
        let got: Vec<u64> = (0..200).map(|_| rng.between([200, 1000])).collect();
        assert!(got.iter().all(|n| (200..=1000).contains(n)));
        assert!(
            got.iter().any(|&n| n < 400) && got.iter().any(|&n| n > 800),
            "两头都取得到"
        );
        assert_eq!(Rng::new(0).next(), Rng::new(1).next(), "0 当 1");
    }
}
