//! `miyu onebot web`（施工 O-28 补，`onebot.md` 第一条「对外的样子」）：跑 `miyu-onebot` 旁边的 `miyu`，参数是
//! `web --package onebot`，标准输入输出照原样接着，退出码照它的；跑不了的照人的语言说、退出码 1。
//!
//! `miyu` 是测试放的替身：Unix 上是一段 shell，Windows 上是一份 `.cmd`，都把参数写进旁边的 `seen`、在标准输出上印一行、照
//! 要求的退出码退出。真的程序找的是自己旁边的 `miyu`（Windows 上带 `.exe`，替身写不出来），那一条只在 Unix 上跑：把
//! `miyu-onebot` 硬链接进 `target/tmp` 里的一个目录，替身放在它旁边。

use std::path::{Path, PathBuf};

use miyu_onebot::texts::Texts;
use miyu_onebot::web::web;
use miyu_store::resources::ResourceRoot;

use crate::support::resources;

/// `target/tmp` 下这一条测试自己的目录（和 `target/debug` 在同一个文件系统上，硬链接得过去），用完删掉。
struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("onebot-web-{name}-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("建得了目录");
        Dir(dir)
    }

    /// 替身印过的参数，一个一行；没跑过的是 `None`。
    fn seen(&self) -> Option<String> {
        let seen = std::fs::read_to_string(self.0.join("seen")).ok()?;
        Some(seen.lines().map(str::trim).collect::<Vec<_>>().join(" "))
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        if std::fs::remove_dir_all(&self.0).is_err() {
            // 删不掉就留在 `target/tmp` 里，不影响测试。
        }
    }
}

/// 在 `dir` 里放一个叫 `name` 的替身（Windows 上另加 `.cmd`）：参数写进 `dir/seen`，标准输出印 `opened`，以 `code` 退出。
/// 交回它的路径。
fn stand_in(dir: &Path, name: &str, code: u8) -> PathBuf {
    let seen = dir.join("seen");
    #[cfg(unix)]
    {
        let path = dir.join(name);
        script(
            &path,
            &format!(
                "#!/bin/sh\nfor arg in \"$@\"; do echo \"$arg\"; done > '{}'\necho opened\nexit {code}\n",
                seen.display()
            ),
        );
        path
    }
    #[cfg(windows)]
    {
        let path = dir.join(format!("{name}.cmd"));
        std::fs::write(
            &path,
            format!(
                "@echo off\r\necho %*> \"{}\"\r\necho opened\r\nexit /b {code}\r\n",
                seen.display()
            ),
        )
        .expect("写得进");
        path
    }
}

/// 把 `body` 写成能跑的 `path`。经 `sh` 写（照 `miyu-core` 测假助手的办法）：这个进程不拿着它的写端，别的测试这时起的进程
/// 带不走，跑它不会撞 ETXTBSY（直接写撞过：同一个测试程序里别的测试正起着进程）。
#[cfg(unix)]
fn script(path: &Path, body: &str) {
    let status = std::process::Command::new("/bin/sh")
        .args(["-c", r#"printf '%s' "$2" > "$1" && chmod 755 "$1""#, "sh"])
        .arg(path)
        .arg(body)
        .status()
        .expect("起得来");
    assert!(status.success());
}

fn zh() -> Texts {
    Texts::load(ResourceRoot::at(resources()), "zh").expect("读得出来")
}

/// 照系统的说法，`program` 为什么跑不了。
fn why_not(program: &Path) -> String {
    std::process::Command::new(program)
        .status()
        .expect_err("跑不了")
        .to_string()
}

#[test]
fn miyu_gets_web_package_onebot_and_its_exit_code_comes_back() {
    for code in [7, 0] {
        let dir = Dir::new("code");
        let miyu = stand_in(&dir.0, "miyu", code);
        let mut err = Vec::new();
        assert_eq!(web(&miyu, &zh(), &mut err), code, "照它的退出码");
        assert_eq!(dir.seen().as_deref(), Some("web --package onebot"));
        assert!(err.is_empty(), "{}", String::from_utf8_lossy(&err));
    }
}

#[test]
fn without_miyu_it_says_so_and_exits_1() {
    let dir = Dir::new("missing");
    let miyu = dir.0.join("miyu");
    let mut err = Vec::new();
    assert_eq!(web(&miyu, &zh(), &mut err), 1);
    assert_eq!(
        String::from_utf8(err).expect("UTF-8"),
        format!(
            "{}\n",
            zh().no_miyu(&miyu.display().to_string(), &why_not(&miyu))
        )
    );
}

#[cfg(unix)]
#[test]
fn a_miyu_stopped_by_a_signal_counts_as_failed() {
    let dir = Dir::new("signal");
    let miyu = dir.0.join("miyu");
    script(&miyu, "#!/bin/sh\nkill -9 $$\n");
    let mut err = Vec::new();
    assert_eq!(web(&miyu, &zh(), &mut err), 1);
    assert!(err.is_empty(), "{}", String::from_utf8_lossy(&err));
}

/// 跑 `dir` 里的 `miyu-onebot web`（数据根是临时的，说中文），交回它的输出。
#[cfg(unix)]
fn run_web(dir: &Dir, args: &[&str]) -> std::process::Output {
    let (home, root) = crate::support::temp_root();
    let program = dir.0.join("miyu-onebot");
    crate::support::spawning::link_beside(Path::new(env!("CARGO_BIN_EXE_miyu-onebot")), &program);
    let output = std::process::Command::new(&program)
        .args(args)
        .env("MIYU_HOME", root.path())
        .env("MIYU_RESOURCES", resources())
        .env("LC_ALL", "zh_CN.UTF-8")
        .stdin(std::process::Stdio::null())
        .output()
        .expect("跑得了");
    if std::fs::remove_dir_all(&home).is_err() {
        // 删不掉就留在临时目录里，不影响测试。
    }
    output
}

#[cfg(unix)]
#[test]
fn the_program_runs_the_miyu_beside_it_and_passes_its_output_through() {
    let dir = Dir::new("beside");
    stand_in(&dir.0, "miyu", 7);
    let ran = run_web(&dir, &["web"]);
    assert_eq!(
        ran.status.code(),
        Some(7),
        "{}",
        String::from_utf8_lossy(&ran.stderr)
    );
    assert_eq!(dir.seen().as_deref(), Some("web --package onebot"));
    assert_eq!(
        String::from_utf8_lossy(&ran.stdout),
        "opened\n",
        "标准输出照原样接着"
    );
    assert_eq!(String::from_utf8_lossy(&ran.stderr), "");
}

#[cfg(unix)]
#[test]
fn the_program_without_miyu_beside_it_says_so_in_the_system_language() {
    let dir = Dir::new("alone");
    let ran = run_web(&dir, &["web"]);
    assert_eq!(ran.status.code(), Some(1));
    let miyu = dir.0.join("miyu");
    assert_eq!(
        String::from_utf8_lossy(&ran.stderr),
        format!(
            "{}\n",
            zh().no_miyu(&miyu.display().to_string(), &why_not(&miyu))
        )
    );
    assert_eq!(String::from_utf8_lossy(&ran.stdout), "");
}
