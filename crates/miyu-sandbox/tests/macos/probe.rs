//! 探测报 `seatbelt`；套在不许再装沙盒的沙盒里（例如 Miyu 自己跑在别的沙盒里）：探测报空的，`run` 收紧不成、命令
//! 没跑，退出 125，最后一行是助手说的那一句。

use std::process::{Command, Stdio};

use miyu_sandbox::{EXIT_HELPER, Probe};

use super::*;
use crate::support::Dir;

/// 外面那一层沙盒：什么都放行，只不许调装沙盒的那个系统调用（`deny default` 的沙盒一般都不许）。
const OUTER: &str = "(version 1)(allow default)(deny system-mac-syscall)";

/// 在外面那一层沙盒里跑助手。
fn nested(args: &[&str]) -> std::process::Output {
    Command::new("/usr/bin/sandbox-exec")
        .args(["-p", OUTER, "--", HELPER])
        .args(args)
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .output()
        .expect("起得来")
}

fn mechanisms(stdout: &[u8]) -> Vec<String> {
    let probe: Probe = serde_json::from_slice(stdout).expect("读得懂");
    probe.mechanisms
}

#[test]
fn the_probe_reports_seatbelt() {
    let out = Command::new(HELPER).arg("probe").output().expect("起得来");
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    assert_eq!(mechanisms(&out.stdout), ["seatbelt"]);
    assert!(out.stderr.is_empty(), "{out:?}");
}

#[test]
fn inside_a_sandbox_that_forbids_another_nothing_is_confined() {
    let probe = nested(&["probe"]);
    assert_eq!(probe.status.code(), Some(0), "{probe:?}");
    assert!(mechanisms(&probe.stdout).is_empty(), "{probe:?}");
    let work = Dir::new();
    let marker = real(work.path()).join("ran");
    let spec = spec(&[&real(work.path())], &[]);
    let marker_text = marker.to_str().expect("路径是 UTF-8");
    let out = nested(&["run", "--spec", &spec, "--", "/usr/bin/touch", marker_text]);
    assert_eq!(out.status.code(), Some(i32::from(EXIT_HELPER)), "{out:?}");
    let stderr = text(&out.stderr);
    // 系统自己先说一句 `sandbox initialization failed: …`，最后一行是助手的。
    assert_eq!(
        stderr.lines().last(),
        Some("miyu-sandbox: cannot confine: Operation not permitted"),
        "{stderr}"
    );
    assert!(!marker.exists(), "命令没跑");
}
