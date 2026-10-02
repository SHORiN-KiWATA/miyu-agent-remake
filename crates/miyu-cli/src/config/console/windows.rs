//! Windows 上关掉回显读一行（施工 8-5 补，`docs/blueprint/cli/login.md`「怎么走」第 4 条）：控制台模式关
//! `ENABLE_ECHO_INPUT`（回显）、`ENABLE_LINE_INPUT`（行缓冲）、`ENABLE_PROCESSED_INPUT`（系统替你处理
//! `Ctrl+C`），自己一个字节一个字节读（[`read_line`]）。
//!
//! 关了 `ENABLE_PROCESSED_INPUT`，`Ctrl+C` 不再触发控制台的打断事件——它变成普通的一个字节（`0x03`），我们自己认出来
//! 当取消；进程不会被打断事件直接打断，`hidden::read_line` 返回以后、不管走哪条路，[`Guard`] 的 `Drop` 都会把控制台
//! 模式照原样写回去，`panic` 也算。
//!
//! 开的是 `CONIN$`（这个进程的控制台输入），不是标准输入：标准输入被重定向了也认得到真的控制台，和以前
//! `rpassword` 的办法一样。

#![allow(unsafe_code, reason = "调 Windows 的控制台接口（施工 8-5 补）")]

use std::fs::{File, OpenOptions};
use std::io::{self, Read};
use std::os::windows::io::AsRawHandle;

use windows_sys::Win32::System::Console::{
    CONSOLE_MODE, ENABLE_ECHO_INPUT, ENABLE_LINE_INPUT, ENABLE_PROCESSED_INPUT, GetConsoleMode,
    SetConsoleMode,
};

use super::hidden::{Step, read_line};

/// 关掉回显读一行。
pub(super) fn read_hidden() -> io::Result<Option<String>> {
    let console = OpenOptions::new().read(true).write(true).open("CONIN$")?;
    let _guard = Guard::enter(&console)?;
    // `(&console).read(...)` 走 `&File` 那个 `Read` 实现，只借一下、不用 `&mut console`：`_guard` 借着 `console`
    // 到这个函数结束（它的 `Drop` 要写回模式），这里要是写成 `console.read(...)` 会跟它的借用撞上。
    read_line(|| {
        let mut byte = [0u8; 1];
        match (&console).read(&mut byte)? {
            0 => Ok(Step::Eof),
            _ => Ok(Step::Byte(byte[0])),
        }
    })
}

/// 存下控制台原来的模式，关回显、行缓冲、`Ctrl+C` 的处理；丢掉的时候（哪条路出去都算，包括 `panic`）照原样写回去。
struct Guard<'a> {
    console: &'a File,
    original: CONSOLE_MODE,
}

impl<'a> Guard<'a> {
    /// 读一遍原来的模式，换上一个字节一个字节读用的模式。
    ///
    /// # Errors
    ///
    /// `console` 不是控制台，或者改不了它的模式。
    fn enter(console: &'a File) -> io::Result<Guard<'a>> {
        let original = mode(console)?;
        let raw = original & !(ENABLE_ECHO_INPUT | ENABLE_LINE_INPUT | ENABLE_PROCESSED_INPUT);
        set_mode(console, raw)?;
        Ok(Guard { console, original })
    }
}

impl Drop for Guard<'_> {
    fn drop(&mut self) {
        if let Err(error) = set_mode(self.console, self.original) {
            tracing::warn!(
                target: "miyu::cli",
                error = %error,
                "console mode not restored after reading a hidden key",
            );
        }
    }
}

/// 读控制台现在的模式。
///
/// # Errors
///
/// `console` 不是控制台的输入句柄。
fn mode(console: &File) -> io::Result<CONSOLE_MODE> {
    let mut mode: CONSOLE_MODE = 0;
    // SAFETY: `console` 是打开的 `CONIN$`，是有效的控制台输入句柄；`mode` 是系统要写的出参，指向一个活着的
    // `CONSOLE_MODE`。
    match unsafe { GetConsoleMode(console.as_raw_handle(), &mut mode) } {
        0 => Err(io::Error::last_os_error()),
        _ => Ok(mode),
    }
}

/// 写控制台的模式。
///
/// # Errors
///
/// `console` 不是控制台的输入句柄，或者 `mode` 不是这个控制台认的位组合。
fn set_mode(console: &File, mode: CONSOLE_MODE) -> io::Result<()> {
    // SAFETY: `console` 是打开的 `CONIN$`，是有效的控制台输入句柄；`mode` 是合法的模式位组合（来自 `mode()` 读到的
    // 原值，或者在它上面关掉几个标准的输入位）。
    match unsafe { SetConsoleMode(console.as_raw_handle(), mode) } {
        0 => Err(io::Error::last_os_error()),
        _ => Ok(()),
    }
}

/// `Guard` 本身和控制台模式的读写，用真的控制台（CI 的 Windows 机器上跑；真按键模拟不了，整段读取流程不在这里测，
/// 施工 8-5 补「验收」第 1 条）：写成一个测试，见下面的注释。
#[cfg(test)]
mod tests {
    use super::*;

    /// 这个进程自己的控制台输入：测试跑在真的控制台下才有。
    fn console() -> io::Result<File> {
        OpenOptions::new().read(true).write(true).open("CONIN$")
    }

    // 模式的读写、`Guard` 丢掉以后照原样写回去，写在同一个测试里：控制台的模式是整个进程共用的一份，不是跟着
    // 文件描述符走的，拆成两个测试并行跑会互相踩——`cargo test` 默认多线程跑，CI 的 Windows 上实测到了
    // （2026-10-02：一个测试在中途正改着模式，另一个测试读到的是半路的值，和自己存的「原来的样子」不一样）。
    #[test]
    fn mode_round_trips_and_guard_restores_it() {
        let Ok(console) = console() else {
            return; // 没有控制台（例如被当成服务跑），这一条跳过。
        };
        let before = mode(&console).expect("读得到");
        set_mode(&console, before).expect("原样写回去也该成");
        assert_eq!(mode(&console).expect("读得到"), before, "读写本身该是对的");

        let watched = ENABLE_ECHO_INPUT | ENABLE_LINE_INPUT | ENABLE_PROCESSED_INPUT;
        {
            let _guard = Guard::enter(&console).expect("进得去");
            let raw = mode(&console).expect("读得到");
            assert_eq!(raw & watched, 0, "三个位都该关着");
        }
        let after = mode(&console).expect("读得到");
        assert_eq!(after, before, "丢掉守卫以后照原样写回去");
    }
}
