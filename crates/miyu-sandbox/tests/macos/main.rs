//! macOS 上真跑助手（`docs/blueprint/sandbox/macos.md`「守着它的」，施工 5-7）：整盘能读，只能写规格里能写的，藏起来
//! 的读写都不行；硬链接、改名、`fcntl` 绕不过去；能替命令在沙盒外读写的系统服务挡住；网络照常，Unix 套接字只连得上
//! 能写的地方；平常的工具照常干活；探测报 `seatbelt`，套在不许再装沙盒的沙盒里收紧不成。只在 macOS 上编。

#![cfg(target_os = "macos")]

#[path = "../support/mod.rs"]
mod support;

mod bypass;
mod child;
mod compat;
mod files;
mod network;
mod probe;

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use support::HELPER;

/// 真实的位置：规格里的路径照它写（macOS 上临时目录在 `/private` 下的链接后面）。
fn real(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).expect("换得了")
}

/// 一份规格的 JSON：`write` 能写，`hidden` 藏起来。照 JSON 写：规格的格以后怎么增减，这里都不用改。
fn spec(write: &[&Path], hidden: &[&Path]) -> String {
    serde_json::json!({ "write": write, "hidden": hidden }).to_string()
}

/// 经助手在 `cwd` 里跑 `program args…`。报错照英文说（`LC_ALL=C`），不跟着跑测试的机器的语言。
fn run(spec: &str, cwd: &Path, program: &str, args: &[&str]) -> Output {
    Command::new(HELPER)
        .env("LC_ALL", "C")
        .current_dir(cwd)
        .args(["run", "--spec", spec, "--", program])
        .args(args)
        .stdin(Stdio::null())
        .output()
        .expect("起得来")
}

/// 经助手在 `cwd` 里跑 `sh -c <script>`。
fn sh(spec: &str, cwd: &Path, script: &str) -> Output {
    run(spec, cwd, "/bin/sh", &["-c", script])
}

/// 测试程序自己当命令，经助手在 `cwd` 里跑：它照 `what` 做一件事（`child.rs`），交回它的标准输出，里面有
/// `child ok` 或者 `child failed: <原话>`。
fn child(spec: &str, cwd: &Path, what: &str) -> String {
    let exe = std::env::current_exe().expect("测试程序在哪");
    let out = Command::new(HELPER)
        .env(child::WHAT, what)
        .current_dir(cwd)
        .args(["run", "--spec", spec, "--"])
        .arg(&exe)
        .args([
            "--exact",
            "child::child",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .stdin(Stdio::null())
        .output()
        .expect("起得来");
    text(&out.stdout)
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// 做成了：退出码 0。交回标准输出。
fn ok(out: &Output) -> String {
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    text(&out.stdout)
}

/// 被沙盒挡住了：没做成，说的是 `Operation not permitted`。
fn denied(out: &Output) {
    assert_ne!(out.status.code(), Some(0), "{out:?}");
    let said = text(&out.stderr) + &text(&out.stdout);
    assert!(said.contains("Operation not permitted"), "{out:?}");
}
