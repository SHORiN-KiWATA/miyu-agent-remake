//! Linux 上真跑助手（`docs/blueprint/sandbox/linux.md`，施工 5-2、5-3）：整盘能读，只管写；一条能写的都没有时只有
//! `/dev/null` 写得进；藏起来的读写都不行，名字看得到；能写的落在藏起来的里面照样能写；藏起来的落在能写的里面拒绝
//! 执行；一级级放行时的链接不跟过去；不在的路径跳过；设了 `no_new_privs`；系统服务的套接字连不上；探测报 `landlock`。
//! 内核没有 Landlock 的机器上，只测拒绝执行。

#![cfg(target_os = "linux")]

mod support;

use std::path::Path;
use std::process::{Command, Output, Stdio};

use miyu_sandbox::{EXIT_HELPER, Probe, Spec};
use support::{Dir, HELPER, serial};

/// 一份规格：`write` 能写，`hidden` 藏起来。
fn spec(write: &[&Path], hidden: &[&Path]) -> Spec {
    Spec {
        write: write.iter().map(|path| path.to_path_buf()).collect(),
        hidden: hidden.iter().map(|path| path.to_path_buf()).collect(),
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

/// 跑 `script`，要它失败，报的是被拦（`Permission denied`）。
fn denied(spec: &Spec, script: &str) {
    let out = run(spec, script);
    assert_ne!(out.status.code(), Some(0), "{script}: {out:?}");
    assert!(
        text(&out.stderr).contains("Permission denied"),
        "{script}: {out:?}"
    );
}

#[test]
fn everything_is_readable_but_only_the_writable_paths_can_be_written() {
    let _serial = serial();
    let work = Dir::new();
    let other = Dir::new();
    let notes = other.file("notes.txt", b"shared notes\n");
    let spec = spec(&[work.path()], &[]);
    if refused_without_landlock(&spec) {
        return;
    }
    // 读：别处的文件、系统的文件、家目录，都读得了。
    let out = run(
        &spec,
        &format!(
            "cat '{}' && head -c 1 /etc/passwd >/dev/null && ls \"$HOME\" >/dev/null && echo read",
            notes.display()
        ),
    );
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(text(&out.stdout), "shared notes\nread\n");
    // 写：只有能写的那一处。
    let file = work.path().join("a.txt");
    // 跨目录的改名、硬链接要 Landlock 第 2 版起的那一样（`mv` 被拒会改成拷贝再删，`ln` 没有退路）。
    let script = format!(
        "echo hi > '{0}' && cat '{0}' && mkdir '{1}/sub' && mv '{0}' '{1}/sub/b.txt' && ln '{1}/sub/b.txt' '{1}/c.txt' && ls '{1}/sub' && cat '{1}/c.txt'",
        file.display(),
        work.path().display()
    );
    let out = run(&spec, &script);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(text(&out.stdout), "hi\nb.txt\nhi\n");
    let home = std::env::var("HOME").expect("有家目录");
    let stray = Path::new(&home).join(format!(".miyu-sandbox-test-{}", std::process::id()));
    for script in [
        format!("echo x > '{}/new.txt'", other.path().display()),
        format!("rm '{}'", notes.display()),
        format!("echo more >> '{}'", notes.display()),
        format!("touch '{}'", stray.display()),
    ] {
        denied(&spec, &script);
    }
    assert_eq!(std::fs::read(&notes).expect("还在"), b"shared notes\n");
    assert!(
        !other.path().join("new.txt").exists() && !stray.exists(),
        "没写进去"
    );
}

#[test]
fn nothing_writable_is_read_only_everywhere_but_dev_null() {
    let _serial = serial();
    let other = Dir::new();
    let spec = spec(&[], &[]);
    if refused_without_landlock(&spec) {
        return;
    }
    let out = run(&spec, "echo x > /dev/null && echo sink");
    assert_eq!(text(&out.stdout), "sink\n", "{out:?}");
    denied(&spec, &format!("touch '{}/x'", other.path().display()));
    let out = run(&spec, "mktemp");
    assert_ne!(out.status.code(), Some(0), "临时目录也写不了：{out:?}");
}

#[test]
fn hidden_paths_cannot_be_read_or_written_but_their_names_show() {
    let _serial = serial();
    let work = Dir::new();
    let data = Dir::new();
    let token = data.file("token", b"secret\n");
    let beside = Dir::new();
    let open = beside.file("open.txt", b"open\n");
    let spec = spec(&[work.path()], &[data.path()]);
    if refused_without_landlock(&spec) {
        return;
    }
    denied(&spec, &format!("cat '{}'", token.display()));
    denied(&spec, &format!("echo x >> '{}'", token.display()));
    let out = run(&spec, &format!("ls '{}'", data.path().display()));
    assert_eq!(text(&out.stdout), "token\n", "名字看得到：{out:?}");
    // 藏起来的旁边照样读得了：一级级放行，只绕开藏起来的那一条。
    let out = run(&spec, &format!("cat '{}'", open.display()));
    assert_eq!(text(&out.stdout), "open\n", "{out:?}");
}

#[test]
fn a_writable_path_inside_a_hidden_one_stays_writable() {
    let _serial = serial();
    let data = Dir::new();
    let workspace = data.path().join("home/admin/workspace");
    std::fs::create_dir_all(&workspace).expect("建得了目录");
    let token = data.file("token", b"secret\n");
    let spec = spec(&[&workspace], &[data.path()]);
    if refused_without_landlock(&spec) {
        return;
    }
    let file = workspace.join("a.txt");
    let out = run(
        &spec,
        &format!("echo hi > '{0}' && cat '{0}'", file.display()),
    );
    assert_eq!(
        text(&out.stdout),
        "hi\n",
        "工作区在数据根里照样能写：{out:?}"
    );
    denied(&spec, &format!("cat '{}'", token.display()));
}

#[test]
fn a_hidden_path_inside_a_writable_one_is_refused_before_running() {
    let _serial = serial();
    let work = Dir::new();
    let spec = spec(&[work.path()], &[&work.path().join("data")]);
    let out = run(&spec, "echo ran");
    assert_eq!(out.status.code(), Some(i32::from(EXIT_HELPER)), "{out:?}");
    assert_eq!(
        text(&out.stderr),
        format!(
            "miyu-sandbox: cannot confine: cannot hide {}/data inside a writable path\n",
            work.path().display()
        )
    );
    assert!(out.stdout.is_empty(), "没跑");
}

#[test]
fn links_on_the_way_to_a_hidden_path_are_not_followed() {
    let _serial = serial();
    let work = Dir::new();
    let base = Dir::new();
    let data = base.path().join("data");
    std::fs::create_dir(&data).expect("建得了目录");
    std::fs::write(data.join("token"), b"secret\n").expect("写得进");
    std::os::unix::fs::symlink(&data, base.path().join("alias")).expect("建得了链接");
    let spec = spec(&[work.path()], &[&data]);
    if refused_without_landlock(&spec) {
        return;
    }
    denied(
        &spec,
        &format!("cat '{}/alias/token'", base.path().display()),
    );
}

#[test]
fn missing_paths_in_the_spec_are_skipped() {
    let _serial = serial();
    let work = Dir::new();
    let gone = work.path().join("not-yet");
    let spec = spec(&[work.path(), &gone], &[Path::new("/no/such/place")]);
    if refused_without_landlock(&spec) {
        return;
    }
    let out = run(&spec, "echo ran");
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(text(&out.stdout), "ran\n");
}

#[test]
fn read_only_files_cannot_be_truncated() {
    let _serial = serial();
    let work = Dir::new();
    let other = Dir::new();
    let notes = other.file("notes.txt", b"shared notes\n");
    let spec = spec(&[work.path()], &[]);
    if refused_without_landlock(&spec) {
        return;
    }
    // 截断：内核 6.2 起 Landlock 管得了 truncate(2)，写不了的也截不了。机器上没有 python3 的不测。
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
fn the_command_runs_with_no_new_privs() {
    let _serial = serial();
    let work = Dir::new();
    let spec = spec(&[work.path()], &[]);
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
    let spec = spec(&[work.path()], &[]);
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
