//! Windows 上跑包的程序（施工 9-2，`docs/blueprint/packages.md`「转交」）：起它、等它，这期间自己不理 Ctrl+C（同一个控制台，
//! 它自己会收到），退出码照它的。Windows 没有换成别的程序那一招（`exec`）。

#![allow(
    unsafe_code,
    reason = "调 Windows 的控制台接口：这期间不理 Ctrl+C（施工 9-2）"
)]

use std::ffi::OsString;
use std::io::Write;
use std::path::Path;

use windows_sys::Win32::System::Console::SetConsoleCtrlHandler;

use super::say;

/// 起 `program args…`、等它，交回它的退出码；起不来的说为什么，退出码 1。
pub(super) fn run(program: &Path, args: &[OsString], err: &mut dyn Write) -> u8 {
    // SAFETY: 处理函数传空、第二个参数是 TRUE：只是让这个进程不理 Ctrl+C，不碰任何内存。
    unsafe {
        SetConsoleCtrlHandler(None, 1);
    }
    match std::process::Command::new(program).args(args).status() {
        Ok(status) => status
            .code()
            .and_then(|code| u8::try_from(code).ok())
            .unwrap_or(1),
        Err(error) => {
            say(err, &format!("{}: {error}", program.display()));
            1
        }
    }
}
