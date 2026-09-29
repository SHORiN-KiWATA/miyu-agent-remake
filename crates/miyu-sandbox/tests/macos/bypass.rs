//! 绕过去：能读不能写的文件，硬链接、符号链接、改名、`fcntl` 都改不了它；能替命令在沙盒外读写的系统服务（打开别的
//! 程序、launchd 的任务、偏好设置、钥匙串）用不了。

use std::fs;
use std::process::Command;

use super::*;
use crate::support::Dir;

#[test]
fn a_readable_file_cannot_be_changed_through_links_or_renames() {
    let work = Dir::new();
    let other = Dir::new();
    let target = real(&other.file("f", b"original\n"));
    let cwd = real(work.path());
    let spec = spec(&[&cwd], &[]);
    let target_text = target.to_str().expect("路径是 UTF-8");
    denied(&run(&spec, &cwd, "/bin/ln", &[target_text, "l"]));
    denied(&sh(
        &spec,
        &cwd,
        &format!("ln -s '{target_text}' s; echo evil >> s"),
    ));
    denied(&run(&spec, &cwd, "/bin/mv", &[target_text, "moved"]));
    // 克隆（`cp -c`）是一份新的：改它不动原来的。
    ok(&sh(
        &spec,
        &cwd,
        &format!("cp -c '{target_text}' c && echo more >> c"),
    ));
    assert_eq!(fs::read(&target).expect("还在"), b"original\n");
}

#[test]
fn fcntl_cannot_change_a_file_opened_read_only() {
    let work = Dir::new();
    let other = Dir::new();
    let target = real(&other.file("f", b"original\n"));
    let cwd = real(work.path());
    let said = child(
        &spec(&[&cwd], &[]),
        &cwd,
        &format!("fcntl-blocked:{}", target.display()),
    );
    assert!(said.contains("child ok"), "{said}");
    assert_eq!(fs::read(&target).expect("还在"), b"original\n");
}

/// 在沙盒外跑一条收拾的命令：收拾不了只说一句，不让测试因为它失败。
fn tidy(program: &str, args: &[&str]) {
    if let Err(error) = Command::new(program).args(args).output() {
        eprintln!("收拾不了（{program}）：{error}");
    }
}

#[test]
fn services_that_could_read_or_write_elsewhere_are_blocked() {
    let work = Dir::new();
    let cwd = real(work.path());
    let spec = spec(&[&cwd], &[]);
    let tag = format!("com.miyu.sandbox.test.{}", std::process::id());
    // 打开别的程序（LaunchServices）：在沙盒外面跑。
    let open = run(&spec, &cwd, "/usr/bin/open", &["-g", "-a", "Finder"]);
    assert_ne!(open.status.code(), Some(0), "{open:?}");
    // launchd 起任务：在沙盒外面跑。
    let submitted = run(
        &spec,
        &cwd,
        "/bin/launchctl",
        &["submit", "-l", &tag, "--", "/usr/bin/true"],
    );
    tidy("/bin/launchctl", &["remove", &tag]);
    assert_ne!(submitted.status.code(), Some(0), "{submitted:?}");
    // 偏好设置：cfprefsd 替命令写 `~/Library/Preferences`。写不写得成看沙盒外读不读得到（`defaults` 写不成时
    // 不一定报错）。
    let written = run(
        &spec,
        &cwd,
        "/usr/bin/defaults",
        &["write", &tag, "key", "value"],
    );
    let kept = Command::new("/usr/bin/defaults")
        .args(["read", &tag, "key"])
        .output()
        .expect("起得来");
    tidy("/usr/bin/defaults", &["delete", &tag]);
    assert_ne!(kept.status.code(), Some(0), "写进去了：{written:?}");
    // 钥匙串：securityd 替命令读写。
    let added = run(
        &spec,
        &cwd,
        "/usr/bin/security",
        &["add-generic-password", "-s", &tag, "-a", "miyu", "-w", "x"],
    );
    let found = Command::new("/usr/bin/security")
        .args(["find-generic-password", "-s", &tag])
        .output()
        .expect("起得来");
    tidy(
        "/usr/bin/security",
        &["delete-generic-password", "-s", &tag],
    );
    assert_ne!(found.status.code(), Some(0), "加进去了：{added:?}");
}
