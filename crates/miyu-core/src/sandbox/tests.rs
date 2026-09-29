//! 探不成的几种都记一行 `WARN sandbox unavailable`（施工 5-1）。跑整套测试时助手总在主程序旁边，找不到的那一行
//! 在真的核心里走不到，在这里直接探一个旁边没有助手的位置。探成了的那一行见 `crates/miyu/tests/core.rs`。
//! 探到了手段的交回能用和助手，手段是空的、找不到、探不成的交回用不了和原因（施工 5-4 上、下）。沙盒的缓存放在缓存目录
//! 下的 `sandbox`，带上你的 cargo 目录；算不出来的记一行（施工 5-4 下）。

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use miyu_log::{LevelFilter, Memory};

use super::*;

/// 探一次 `exe`，交回探到的结果和记下的行。
fn probed(exe: Option<&Path>) -> (Availability, Vec<String>) {
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
        let (found, lines) = probed(exe.as_deref());
        assert_eq!(found, Availability::Unusable(Unusable::HelperMissing));
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
    let (found, lines) = probed(Some(&dir.exe()));
    assert_eq!(found, Availability::Unusable(Unusable::HelperFailed));
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
    let (found, lines) = probed(Some(&dir.exe()));
    assert_eq!(
        found,
        Availability::Usable(dir.0.join(miyu_sandbox::HELPER)),
        "{lines:?}"
    );
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
    let (found, lines) = probed(Some(&dir.exe()));
    assert_eq!(
        found,
        Availability::Unusable(Unusable::NoMechanism),
        "照沙盒用不了办：{lines:?}"
    );
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(
        lines[0].contains(" INFO ") && lines[0].contains(" sandbox helper="),
        "{lines:?}"
    );
}

/// 根目录下面的 `parts`：Windows 上得带盘符才算绝对路径。
fn at(parts: &[&str]) -> PathBuf {
    let root = PathBuf::from(if cfg!(windows) { "C:\\" } else { "/" });
    parts.iter().fold(root, |path, part| path.join(part))
}

/// 一份环境：Linux 的写法，家目录是 `home`，别的都没设。
fn linux(home: Option<PathBuf>) -> Env {
    Env {
        platform: miyu_store::env::Platform::Linux,
        miyu_home: None,
        home,
        xdg_cache_home: None,
        local_app_data: None,
        miyu_resources: None,
        exe: None,
    }
}

/// 照 `env`、`cargo_home` 算一次沙盒的缓存，交回算出来的和记下的行。
fn cached(
    env: &Env,
    cargo_home: Option<std::ffi::OsString>,
) -> (Option<(PathBuf, Option<PathBuf>)>, Vec<String>) {
    let memory = Memory::new();
    let found = tracing::subscriber::with_default(
        miyu_log::subscriber(memory.clone(), LevelFilter::INFO, None),
        || cache(env, cargo_home),
    );
    (found, memory.lines())
}

#[test]
fn the_sandbox_cache_is_under_the_cache_dir_and_knows_your_cargo() {
    let home = at(&["home", "alice"]);
    let env = linux(Some(home.clone()));
    let root = home.join(".cache").join("miyu").join("sandbox");
    let (found, lines) = cached(&env, None);
    assert_eq!(found, Some((root.clone(), Some(home.join(".cargo")))));
    assert!(lines.is_empty(), "{lines:?}");
    // `CARGO_HOME` 设了的照它；空的当没设。
    let cargo = at(&["opt", "cargo"]);
    assert_eq!(
        cached(&env, Some(cargo.clone().into_os_string())).0,
        Some((root.clone(), Some(cargo)))
    );
    assert_eq!(
        cached(&env, Some(std::ffi::OsString::new())).0,
        Some((root, Some(home.join(".cargo"))))
    );
}

#[test]
fn no_cache_dir_is_logged_and_gives_none() {
    let (found, lines) = cached(&linux(None), Some(at(&["opt", "cargo"]).into_os_string()));
    assert_eq!(found, None);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(
        lines[0].contains(" WARN ")
            && lines[0].ends_with(" sandbox cache unavailable reason=\"no home directory\""),
        "{lines:?}"
    );
}
