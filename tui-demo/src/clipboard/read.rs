//! 读系统剪贴板里的字（`Ctrl+V`，蓝图 `tui.md`「按键」）：按平台依次试剪贴板命令，第一个读得到的算数，
//! 每个最多等一会儿（卡住的剪贴板命令不能把界面也卡住）。只读字，图片以后再说。
//!
//! 经 SSH 连过来时读的是那台机器的剪贴板；要粘手边的东西，用终端自己的粘贴（走括号粘贴，不经这里）。

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// 一个剪贴板命令最多等多久。
pub(super) const WAIT: Duration = Duration::from_secs(1);

/// 读不到：这台机器上没有能用的剪贴板命令，或者都失败了、卡住了。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unreadable;

/// 读系统剪贴板里的字。剪贴板是空的交回空字符串。
///
/// # Errors
///
/// 每个剪贴板命令都读不到（没装、失败、卡住）时交回 [`Unreadable`]。
pub fn read() -> Result<String, Unreadable> {
    let os = if cfg!(target_os = "macos") {
        Os::Mac
    } else if cfg!(windows) {
        Os::Windows
    } else {
        Os::Linux
    };
    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
    read_with(&candidates(os, wayland), WAIT)
}

/// 哪种系统。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Os {
    Linux,
    Mac,
    Windows,
}

/// 这种系统上依次试哪些命令（命令、参数）。Linux 有 Wayland 的先试 `wl-paste`。
pub(super) fn candidates(os: Os, wayland: bool) -> Vec<(&'static str, &'static [&'static str])> {
    match os {
        Os::Mac => vec![("pbpaste", &[])],
        Os::Windows => vec![(
            "powershell",
            &["-NoProfile", "-Command", "Get-Clipboard -Raw"],
        )],
        Os::Linux => {
            let mut list: Vec<(&str, &[&str])> = Vec::new();
            if wayland {
                list.push(("wl-paste", &["--no-newline"]));
            }
            list.push(("xclip", &["-selection", "clipboard", "-o"]));
            list.push(("xsel", &["--clipboard", "--output"]));
            list
        }
    }
}

/// 依次试 `list` 里的命令，第一个成功退出的算数；每个最多等 `wait`。
pub(super) fn read_with(list: &[(&str, &[&str])], wait: Duration) -> Result<String, Unreadable> {
    list.iter()
        .find_map(|(cmd, args)| run(cmd, args, wait))
        .ok_or(Unreadable)
}

/// 跑一个命令，读它的输出当字（照 UTF-8，认不出的字换成替代符）。
fn run(cmd: &str, args: &[&str], wait: Duration) -> Option<String> {
    run_bytes(cmd, args, wait).map(|out| String::from_utf8_lossy(&out).into_owned())
}

/// 跑一个命令，读它的输出；没装、失败、到点还没完的交回 `None`（到点的杀掉）。输出边跑边在另一个线程里读：
/// 剪贴板里东西多时（超过管道的缓冲），命令要等人读走才写得完、才会退出。
pub(super) fn run_bytes(cmd: &str, args: &[&str], wait: Duration) -> Option<Vec<u8>> {
    let mut child = Command::new(cmd)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdout = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut out = Vec::new();
        stdout.read_to_end(&mut out).map(|_| out)
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if start.elapsed() < wait => std::thread::sleep(Duration::from_millis(5)),
            _ => {
                // 杀不掉、等不到也没别的办法：到点就当读不到。
                child.kill().unwrap_or_default();
                child.wait().map(drop).unwrap_or_default();
                break None;
            }
        }
    };
    let out = reader.join().ok()?.ok()?;
    status
        .filter(std::process::ExitStatus::success)
        .map(|_| out)
}

#[cfg(test)]
mod tests;
