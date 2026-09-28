//! 探不成的几种都记一行 `WARN sandbox unavailable`（施工 5-1）。跑整套测试时助手总在主程序旁边，找不到的那一行
//! 在真的核心里走不到，在这里直接探一个旁边没有助手的位置。探成了的那一行见 `crates/miyu/tests/core.rs`。

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use miyu_log::{LevelFilter, Memory};

use super::*;

/// 探一次 `exe`，交回记下的行。
fn logged(exe: Option<&Path>) -> Vec<String> {
    let memory = Memory::new();
    tracing::subscriber::with_default(
        miyu_log::subscriber(memory.clone(), LevelFilter::INFO, None),
        || probe(exe),
    );
    memory.lines()
}

/// 一个用完就删的临时目录，里面有一个假的主程序 `miyu`。
struct Dir(PathBuf);

impl Dir {
    fn new() -> Dir {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("miyu-core-sandbox-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("建得了目录");
        std::fs::write(dir.join("miyu"), b"").expect("写得进");
        Dir(dir)
    }

    fn exe(&self) -> PathBuf {
        self.0.join("miyu")
    }
}

impl Drop for Dir {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "删不掉就留在临时目录里，不影响测试"
    )]
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn no_helper_beside_the_program_is_logged_as_not_found() {
    let dir = Dir::new();
    for exe in [None, Some(dir.exe())] {
        let lines = logged(exe.as_deref());
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(
            lines[0].contains(" WARN ")
                && lines[0].ends_with(" sandbox unavailable reason=\"helper not found\""),
            "{lines:?}"
        );
    }
}

#[test]
fn a_helper_that_cannot_run_is_logged_with_the_reason() {
    let dir = Dir::new();
    std::fs::write(dir.0.join(miyu_sandbox::HELPER), b"just text\n").expect("写得进");
    let lines = logged(Some(&dir.exe()));
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(
        lines[0].contains(" WARN ")
            && lines[0].contains(" sandbox unavailable reason=\"cannot run helper: "),
        "{lines:?}"
    );
}
