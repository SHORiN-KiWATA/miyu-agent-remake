//! 几份测试共用的：编出来的助手在哪，一个用完就删的临时目录。

#![allow(dead_code, reason = "几份测试各用其中几样")]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

/// cargo 编出来的助手。
pub const HELPER: &str = env!("CARGO_BIN_EXE_miyu-sandbox");

/// 写了脚本马上执行的几份测试排着跑：别的线程恰好在这时起子进程，子进程带着还没关的写句柄，执行脚本就会碰上
/// 「文件正忙」（Linux 的 `ETXTBSY`）。同一份测试里起子进程的都先拿它。
pub fn serial() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(PoisonError::into_inner)
}

/// 一个用完就删的临时目录。
pub struct Dir(pub PathBuf);

impl Dir {
    pub fn new() -> Dir {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("miyu-sandbox-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("建得了目录");
        Dir(dir)
    }

    /// 在目录里写一个文件。
    pub fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, bytes).expect("写得进");
        path
    }

    /// 在目录里写一个能执行的脚本（Unix）。
    #[cfg(unix)]
    pub fn script(&self, name: &str, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = self.file(name, format!("#!/bin/sh\n{body}\n").as_bytes());
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("设得了权限");
        path
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) {
            eprintln!("临时目录没删掉：{error}");
        }
    }
}
