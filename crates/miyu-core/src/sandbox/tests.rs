//! 探不成的几种都记一行 `WARN sandbox unavailable`（施工 5-1）。跑整套测试时助手总在主程序旁边，找不到的那一行
//! 在真的核心里走不到，在这里直接探一个旁边没有助手的位置。探成了的那一行见 `crates/miyu/tests/core.rs`。
//! 探到了手段的才交回助手，手段是空的、探不成的交回空的（施工 5-4 上）。

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use miyu_log::{LevelFilter, Memory};

use super::*;

/// 探一次 `exe`，交回交出来的助手和记下的行。
fn probed(exe: Option<&Path>) -> (Option<PathBuf>, Vec<String>) {
    let memory = Memory::new();
    let helper = tracing::subscriber::with_default(
        miyu_log::subscriber(memory.clone(), LevelFilter::INFO, None),
        || probe(exe),
    );
    (helper, memory.lines())
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
        let (helper, lines) = probed(exe.as_deref());
        assert_eq!(helper, None);
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
    let (helper, lines) = probed(Some(&dir.exe()));
    assert_eq!(helper, None);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(
        lines[0].contains(" WARN ")
            && lines[0].contains(" sandbox unavailable reason=\"cannot run helper: "),
        "{lines:?}"
    );
}

/// 在 `dir` 里放一个假的助手：`probe` 时印 `line`。经 `sh` 写：这个进程不拿着它的写端，别的测试这时起进程，也不会让
/// 它执行不了（ETXTBSY）。
#[cfg(unix)]
fn fake_helper(dir: &Dir, line: &str) {
    let status = std::process::Command::new("/bin/sh")
        .args([
            "-c",
            r#"printf '#!/bin/sh\ncat <<"EOF"\n%s\nEOF\n' "$2" > "$1" && chmod 755 "$1""#,
            "sh",
        ])
        .arg(dir.0.join(miyu_sandbox::HELPER))
        .arg(line)
        .status()
        .expect("起得来");
    assert!(status.success());
}

/// 助手说的一行：这一版、这台机器的平台、手段 `mechanisms`。
#[cfg(unix)]
fn said(mechanisms: &[&str]) -> String {
    serde_json::json!({
        "version": miyu_sandbox::VERSION,
        "platform": miyu_sandbox::Platform::current().name(),
        "mechanisms": mechanisms,
    })
    .to_string()
}

#[cfg(unix)]
#[test]
fn a_helper_with_mechanisms_is_handed_on() {
    let dir = Dir::new();
    fake_helper(&dir, &said(&["landlock"]));
    let (helper, lines) = probed(Some(&dir.exe()));
    assert_eq!(helper, Some(dir.0.join(miyu_sandbox::HELPER)), "{lines:?}");
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(
        lines[0].contains(" INFO ") && lines[0].ends_with(" mechanisms=landlock"),
        "{lines:?}"
    );
}

#[cfg(unix)]
#[test]
fn a_helper_without_mechanisms_is_not_handed_on() {
    let dir = Dir::new();
    fake_helper(&dir, &said(&[]));
    let (helper, lines) = probed(Some(&dir.exe()));
    assert_eq!(helper, None, "照沙盒用不了办：{lines:?}");
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(
        lines[0].contains(" INFO ") && lines[0].contains(" sandbox helper="),
        "{lines:?}"
    );
}
