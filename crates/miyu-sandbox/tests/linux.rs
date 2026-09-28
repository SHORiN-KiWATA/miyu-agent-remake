//! Linux 上真跑助手（`docs/blueprint/sandbox/linux.md`，施工 5-2）：规格里的读得了、写得了；规格外的读不了、写不了、
//! 执行不了，命令起的子进程一样；只读目录里写不了；不在的路径跳过；只读的、藏起来的落在放行范围里拒绝执行；探测
//! 报 `landlock`；收紧时设了 `no_new_privs`。内核没有 Landlock 的机器上，只测拒绝执行。

#![cfg(target_os = "linux")]

mod support;

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use miyu_sandbox::{EXIT_HELPER, Probe, Spec};
use support::{Dir, HELPER, serial};

/// 系统目录：`sh`、`cat` 这些要读的程序和库（设计 11 第四节的第一版清单）。不在的助手跳过。
const SYSTEM: [&str; 8] = [
    "/usr", "/bin", "/sbin", "/lib", "/lib32", "/lib64", "/etc", "/opt",
];

/// 一份规格：系统目录加 `read` 能读，`write` 能写，`/dev/null` 能写。
fn spec(read: &[&Path], write: &[&Path]) -> Spec {
    let mut all_read: Vec<PathBuf> = SYSTEM.iter().map(PathBuf::from).collect();
    all_read.extend(read.iter().map(|path| path.to_path_buf()));
    let mut all_write: Vec<PathBuf> = write.iter().map(|path| path.to_path_buf()).collect();
    all_write.push(PathBuf::from("/dev/null"));
    Spec {
        read: all_read,
        write: all_write,
        readonly: Vec::new(),
        hidden: Vec::new(),
    }
}

/// 经助手跑 `sh -c <script>`。报错照英文说（`LC_ALL=C`），不跟着跑测试的机器的语言。
fn run(spec: &Spec, script: &str) -> Output {
    Command::new(HELPER)
        .env("LC_ALL", "C")
        .args([
            "run",
            "--spec",
            &spec.to_json().expect("写得成"),
            "--",
            "/bin/sh",
            "-c",
            script,
        ])
        .stdin(Stdio::null())
        .output()
        .expect("起得来")
}

/// 这台机器的内核有没有能用的 Landlock：问助手自己。
fn landlock() -> bool {
    let out = Command::new(HELPER).arg("probe").output().expect("起得来");
    let probe: Probe = serde_json::from_slice(&out.stdout).expect("读得懂");
    probe.mechanisms.iter().any(|name| name == "landlock")
}

/// 没有 Landlock 的机器上：拒绝执行，交回 `true`，这个测试到此为止。
fn refused_without_landlock(spec: &Spec) -> bool {
    if landlock() {
        return false;
    }
    let out = run(spec, "echo ran");
    assert_eq!(out.status.code(), Some(i32::from(EXIT_HELPER)), "{out:?}");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.starts_with("miyu-sandbox: cannot confine: landlock is not available: "),
        "{stderr}"
    );
    assert!(out.stdout.is_empty(), "没跑");
    true
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn allowed_paths_can_be_read_and_written_by_the_command_and_its_children() {
    let _serial = serial();
    let work = Dir::new();
    let spec = spec(&[], &[work.path()]);
    if refused_without_landlock(&spec) {
        return;
    }
    let file = work.path().join("a.txt");
    // 跨目录的改名、硬链接要 Landlock 第 2 版起的那一样（`mv` 被拒会改成拷贝再删，`ln` 没有退路）。
    let script = format!(
        "echo hi > '{0}' && cat '{0}' && mkdir '{1}/sub' && mv '{0}' '{1}/sub/b.txt' && ln '{1}/sub/b.txt' '{1}/c.txt' && ls '{1}/sub' 2>/dev/null && cat '{1}/c.txt'",
        file.display(),
        work.path().display()
    );
    let out = run(&spec, &script);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(text(&out.stdout), "hi\nb.txt\nhi\n");
}

#[test]
fn paths_outside_the_spec_cannot_be_read_written_or_listed() {
    let _serial = serial();
    let work = Dir::new();
    let other = Dir::new();
    let secret = other.file("secret.txt", b"do not read\n");
    let spec = spec(&[], &[work.path()]);
    if refused_without_landlock(&spec) {
        return;
    }
    for script in [
        format!("cat '{}'", secret.display()),
        format!("echo x > '{}/new.txt'", other.path().display()),
        format!("ls '{}'", other.path().display()),
        format!("rm '{}'", secret.display()),
    ] {
        let out = run(&spec, &script);
        assert_ne!(out.status.code(), Some(0), "{script}: {out:?}");
        assert!(
            text(&out.stderr).contains("Permission denied"),
            "{script}: {out:?}"
        );
    }
    assert!(secret.exists(), "没删掉");
    assert!(!other.path().join("new.txt").exists(), "没写进去");
}

#[test]
fn read_paths_are_read_only() {
    let _serial = serial();
    let shared = Dir::new();
    let notes = shared.file("notes.txt", b"shared notes\n");
    let spec = spec(&[shared.path()], &[]);
    if refused_without_landlock(&spec) {
        return;
    }
    let out = run(&spec, &format!("cat '{}'", notes.display()));
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(text(&out.stdout), "shared notes\n");
    for script in [
        format!("echo more >> '{}'", notes.display()),
        format!("touch '{}/new.txt'", shared.path().display()),
        format!("rm '{}'", notes.display()),
    ] {
        let out = run(&spec, &script);
        assert_ne!(out.status.code(), Some(0), "{script}: {out:?}");
    }
    assert_eq!(std::fs::read(&notes).expect("还在"), b"shared notes\n");
    // 截断：内核 6.2 起 Landlock 管得了 truncate(2)，只读的截不了。机器上没有 python3 的不测这一条。
    let script = format!(
        "command -v python3 >/dev/null || exit 77; python3 -c 'import os, sys; os.truncate(sys.argv[1], 0)' '{}'",
        notes.display()
    );
    let out = run(&spec, &script);
    if out.status.code() != Some(77) {
        assert_ne!(out.status.code(), Some(0), "截不了：{out:?}");
        assert_eq!(std::fs::read(&notes).expect("还在"), b"shared notes\n");
    }
}

#[test]
fn programs_outside_the_spec_cannot_run() {
    let _serial = serial();
    let work = Dir::new();
    let tools = Dir::new();
    let copied = tools.path().join("true");
    std::fs::copy("/bin/true", &copied).expect("拷得了");
    let spec = spec(&[], &[work.path()]);
    if refused_without_landlock(&spec) {
        return;
    }
    let out = Command::new(HELPER)
        .args(["run", "--spec", &spec.to_json().expect("写得成"), "--"])
        .arg(&copied)
        .output()
        .expect("起得来");
    assert_eq!(out.status.code(), Some(126), "{out:?}");
    assert!(
        text(&out.stderr).starts_with(&format!("miyu-sandbox: cannot run {}: ", copied.display())),
        "{out:?}"
    );
}

#[test]
fn carve_outs_inside_allowed_paths_are_refused_before_running() {
    let _serial = serial();
    let work = Dir::new();
    let mut readonly = spec(&[], &[work.path()]);
    readonly.readonly.push(work.path().join(".git"));
    let mut hidden = spec(&[], &[work.path()]);
    hidden.hidden.push(work.path().join("data"));
    for (spec, said) in [
        (
            readonly,
            format!(
                "miyu-sandbox: cannot confine: cannot keep {}/.git read-only inside a writable path\n",
                work.path().display()
            ),
        ),
        (
            hidden,
            format!(
                "miyu-sandbox: cannot confine: cannot hide {}/data inside an allowed path\n",
                work.path().display()
            ),
        ),
    ] {
        let out = run(&spec, "echo ran");
        assert_eq!(out.status.code(), Some(i32::from(EXIT_HELPER)), "{out:?}");
        assert_eq!(text(&out.stderr), said);
        assert!(out.stdout.is_empty(), "没跑");
    }
}

#[test]
fn readonly_and_hidden_outside_writable_paths_are_kept() {
    let _serial = serial();
    let work = Dir::new();
    let docs = Dir::new();
    let data = Dir::new();
    let readme = docs.file("README", b"read me\n");
    let token = data.file("token", b"secret\n");
    let mut spec = spec(&[], &[work.path()]);
    spec.readonly.push(docs.path().to_path_buf());
    spec.hidden.push(data.path().to_path_buf());
    if refused_without_landlock(&spec) {
        return;
    }
    let out = run(&spec, &format!("cat '{}'", readme.display()));
    assert_eq!(text(&out.stdout), "read me\n", "只读的读得了：{out:?}");
    let out = run(&spec, &format!("echo x >> '{}'", readme.display()));
    assert_ne!(out.status.code(), Some(0), "只读的写不了：{out:?}");
    let out = run(&spec, &format!("cat '{}'", token.display()));
    assert_ne!(out.status.code(), Some(0), "藏起来的读不了：{out:?}");
}

#[test]
fn missing_paths_in_the_spec_are_skipped() {
    let _serial = serial();
    let work = Dir::new();
    let gone = work.path().join("not-yet");
    let spec = spec(&[Path::new("/no/such/place")], &[work.path(), &gone]);
    if refused_without_landlock(&spec) {
        return;
    }
    let out = run(&spec, "echo ran");
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(text(&out.stdout), "ran\n");
}

#[test]
fn the_command_runs_with_no_new_privs() {
    let _serial = serial();
    let work = Dir::new();
    let spec = spec(&[Path::new("/proc")], &[work.path()]);
    if refused_without_landlock(&spec) {
        return;
    }
    let out = run(&spec, "grep NoNewPrivs /proc/self/status");
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(
        text(&out.stdout).split_whitespace().collect::<Vec<_>>(),
        ["NoNewPrivs:", "1"]
    );
}

#[test]
fn the_probe_reports_landlock_when_the_kernel_has_it() {
    let has = std::fs::read_to_string("/sys/kernel/security/lsm")
        .map(|list| list.split(',').any(|name| name.trim() == "landlock"))
        .unwrap_or(false);
    // 内核启用了 Landlock（`/sys/kernel/security/lsm` 里有它）的，探测一定报它；读不到这份清单的不下结论。
    if has {
        assert!(landlock(), "内核启用了 Landlock，探测却没报");
    }
}

/// 这台机器的内核版本：`7.2.3-zen1` 这样的写法取前两段。
fn kernel() -> (u32, u32) {
    let release = std::fs::read_to_string("/proc/sys/kernel/osrelease").expect("读得出");
    let mut parts = release
        .trim()
        .split(['.', '-'])
        .map(|part| part.parse().unwrap_or(0));
    (parts.next().unwrap_or(0), parts.next().unwrap_or(0))
}

/// 在沙盒里用 `python3` 连一个 Unix 套接字，`address` 以 `@` 开头的是抽象的。交回连得上连不上；机器上没有 `python3`
/// 的交回空的。
fn connects(spec: &Spec, address: &str) -> Option<bool> {
    let script = format!(
        "command -v python3 >/dev/null || exit 77; python3 -c 'import socket, sys; a = sys.argv[1]; s = socket.socket(socket.AF_UNIX); s.connect(\"\\0\" + a[1:] if a.startswith(\"@\") else a)' '{address}'"
    );
    let out = run(spec, &script);
    match out.status.code() {
        Some(77) => None,
        Some(0) => Some(true),
        _ => {
            let stderr = text(&out.stderr);
            assert!(
                stderr.contains("PermissionError")
                    || stderr.contains("Operation not permitted")
                    || stderr.contains("Permission denied"),
                "连不上的原因不是被拦：{stderr}"
            );
            Some(false)
        }
    }
}

/// 能替命令在沙盒外读写的系统服务照样挡（2026-09-29 项目主人定）：规格外路径上的 Unix 套接字连不上（内核 7.1 起），
/// 沙盒外建的抽象套接字连不上（6.12 起）；更老的内核拦不住，照连得上测。能写的地方的套接字照样连得上。
#[test]
fn system_service_sockets_outside_the_spec_cannot_be_connected() {
    use std::os::linux::net::SocketAddrExt;
    use std::os::unix::net::{SocketAddr, UnixListener};
    let _serial = serial();
    let work = Dir::new();
    let other = Dir::new();
    let spec = spec(&[], &[work.path()]);
    if refused_without_landlock(&spec) {
        return;
    }
    let outside = other.path().join("bus");
    let _outside = UnixListener::bind(&outside).expect("听得了");
    let inside = work.path().join("mine.sock");
    let _inside = UnixListener::bind(&inside).expect("听得了");
    let name = format!("miyu-sandbox-test-{}", std::process::id());
    let _abstract = UnixListener::bind_addr(
        &SocketAddr::from_abstract_name(name.as_bytes()).expect("名字合法"),
    )
    .expect("听得了");
    let version = kernel();
    let Some(outside_ok) = connects(&spec, &outside.to_string_lossy()) else {
        return;
    };
    assert_eq!(
        outside_ok,
        version < (7, 1),
        "规格外路径上的套接字，内核 {version:?}"
    );
    assert_eq!(
        connects(&spec, &inside.to_string_lossy()),
        Some(true),
        "能写的地方的照样连得上"
    );
    assert_eq!(
        connects(&spec, &format!("@{name}")),
        Some(version < (6, 12)),
        "沙盒外建的抽象套接字，内核 {version:?}"
    );
}

/// CI 上一定要真跑 Landlock：没有的话，上面几条只测了拒绝执行，Linux 的沙盒等于没测到。GitHub 的机器设了 `CI`。
#[test]
fn ci_machines_run_with_landlock() {
    if std::env::var_os("CI").is_some() {
        assert!(
            landlock(),
            "CI 的内核没有能用的 Landlock，Linux 的沙盒没真测到"
        );
    }
}
