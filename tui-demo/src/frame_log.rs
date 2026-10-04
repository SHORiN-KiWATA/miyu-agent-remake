//! 每一帧花了多久（蓝图 `tui.md`「环境变量」的 `MIYU_TUI_FRAME_LOG`，设计 23「量界面」）：给了文件路径，每画一帧往里
//! 追加一行 `距启动的毫秒 准备的微秒 整帧的微秒`；整帧超过 `slow_frame_ms` 的，后面接着写各块各花了多少微秒
//! （`块=微秒`，[`section`] 量的），好找慢在哪。不给就什么都不做。只是量性能用的，写不进去也不打扰界面。

use std::cell::RefCell;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant};

thread_local! {
    /// 这一帧各块花了多久：开着记帧时每帧开头清空，[`section`] 往里记。
    static SECTIONS: RefCell<Option<Vec<(&'static str, Duration)>>> = const { RefCell::new(None) };
}

/// 量一块（`name` 是块的名字）：开着记帧时记下 `f` 花了多久，不然直接跑 `f`。
pub fn section<T>(name: &'static str, f: impl FnOnce() -> T) -> T {
    if !SECTIONS.with(|s| s.borrow().is_some()) {
        return f();
    }
    let start = Instant::now();
    let out = f();
    let took = start.elapsed();
    SECTIONS.with(|s| {
        if let Some(list) = s.borrow_mut().as_mut() {
            list.push((name, took));
        }
    });
    out
}

/// 记帧的文件、启动的时刻、多慢的帧要写各块。
#[derive(Debug)]
pub struct FrameLog {
    out: Option<File>,
    start: Instant,
    slow: Duration,
}

impl FrameLog {
    /// 照 `path` 开（追加写）；没给、开不了的什么都不记。整帧超过 `slow` 的写上各块。
    pub fn open(path: Option<&Path>, slow: Duration) -> Self {
        let out = path.and_then(|p| File::options().create(true).append(true).open(p).ok());
        Self {
            out,
            start: Instant::now(),
            slow,
        }
    }

    /// 一帧开头：开着记帧时清空各块的记录，开始量。
    pub fn begin(&self) {
        if self.on() {
            SECTIONS.with(|s| *s.borrow_mut() = Some(Vec::new()));
        }
    }

    /// 要不要量：没开文件的连计时都省掉。
    pub fn on(&self) -> bool {
        self.out.is_some()
    }

    /// 记一帧：准备用了 `prep`，整帧用了 `total`；慢的接着写各块。写不进去就不再写。
    pub fn record(&mut self, prep: Duration, total: Duration) {
        let at = self.start.elapsed();
        let sections = SECTIONS.with(|s| s.borrow_mut().take()).unwrap_or_default();
        let sections = if total >= self.slow {
            sections
        } else {
            Vec::new()
        };
        if let Some(out) = self.out.as_mut()
            && out
                .write_all(line(at, prep, total, &sections).as_bytes())
                .is_err()
        {
            self.out = None;
        }
    }
}

/// 一帧一行：`距启动的毫秒 准备的微秒 整帧的微秒`，慢的后面接 `块=微秒`。
fn line(at: Duration, prep: Duration, total: Duration, sections: &[(&str, Duration)]) -> String {
    let mut out = format!(
        "{} {} {}",
        at.as_millis(),
        prep.as_micros(),
        total.as_micros()
    );
    for (name, took) in sections {
        out.push_str(&format!(" {name}={}", took.as_micros()));
    }
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{FrameLog, line};

    #[test]
    fn one_frame_is_one_line_of_three_numbers() {
        assert_eq!(
            line(
                Duration::from_millis(1234),
                Duration::from_micros(850),
                Duration::from_micros(2100),
                &[]
            ),
            "1234 850 2100\n"
        );
        assert_eq!(
            line(
                Duration::from_millis(9),
                Duration::from_micros(30_000),
                Duration::from_micros(31_000),
                &[("body", Duration::from_micros(29_000))]
            ),
            "9 30000 31000 body=29000\n",
            "慢的帧写上各块"
        );
    }

    #[test]
    fn without_a_path_nothing_is_recorded() {
        let mut log = FrameLog::open(None, Duration::from_millis(8));
        assert!(!log.on());
        log.record(Duration::ZERO, Duration::ZERO);
    }
}
