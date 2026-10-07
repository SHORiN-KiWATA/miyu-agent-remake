//! 用完就删的临时目录（施工 R-3 下从 `mod.rs` 挪出来：那边放不下了）。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// 一个用完就删的临时目录。
pub struct Scratch(pub PathBuf);

impl Scratch {
    pub fn new() -> Scratch {
        Scratch::under(&std::env::temp_dir())
    }

    /// 放在 cargo 给集成测试的 `target/tmp` 下面，不在系统的临时目录里（施工 4-3 下）：临时目录整个能读能写，
    /// 放在里面就造不出「边界以外」。
    pub fn outside_temp() -> Scratch {
        Scratch::under(Path::new(env!("CARGO_TARGET_TMPDIR")))
    }

    fn under(dir: &Path) -> Scratch {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        Scratch(dir.join(format!(
            "miyu-session-{}-{}-{n}",
            std::process::id(),
            stamp()
        )))
    }
}

impl Drop for Scratch {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "删不掉就留在临时目录里，不影响测试"
    )]
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// 纳秒时刻：临时目录名里加上它，Windows 很快复用进程号，光靠进程号和序号会撞上前一个测试进程留下的目录。
fn stamp() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos())
}
