//! 测试用的：临时目录。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// 测试用的临时目录：进程号加序号，用完删掉。
pub(crate) struct Scratch(PathBuf);

impl Scratch {
    pub(crate) fn new() -> Scratch {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        Scratch(std::env::temp_dir().join(format!("miyu-store-test-{}-{n}", std::process::id())))
    }

    /// 这个临时目录的路径（还没建）。
    pub(crate) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "删不掉就留在临时目录里，不影响测试"
    )]
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
