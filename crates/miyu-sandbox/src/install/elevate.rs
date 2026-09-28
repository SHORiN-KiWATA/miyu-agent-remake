//! 查是不是管理员、起提升过的自己（`docs/blueprint/sandbox/windows.md`「怎么走」第 3 条）。
//!
//! 是不是管理员，查的是当前的令牌里 `Administrators` 组启用了没有：UAC 下没提升的管理员，令牌里这个组只用来拒绝，
//! 也算不是。不是的，拿 `runas` 起一个提升过的自己（`ShellExecuteExW`）：系统弹一次 UAC，人点了同意才起得来。窗口
//! 藏起来，出错不弹框；等它退出，交回它的退出码。
//!
//! 这是放开 `unsafe` 的几个模块之一（施工 5-8）：每个 `unsafe` 块前面写着为什么安全。

#![allow(unsafe_code, reason = "调 Windows 的系统接口（施工 5-8）")]

use std::ffi::OsString;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::ptr;

use windows_sys::Win32::Foundation::{ERROR_CANCELLED, WAIT_FAILED};
use windows_sys::Win32::Security::{CheckTokenMembership, WinBuiltinAdministratorsSid};
use windows_sys::Win32::System::Threading::{GetExitCodeProcess, INFINITE, WaitForSingleObject};
use windows_sys::Win32::UI::Shell::{
    SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
    ShellExecuteExW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;

use super::winsys::{Handle, Sid, wide};
use super::{Elevation, quote};

/// 当前是不是管理员：令牌里 `Administrators` 组启用着。
///
/// # Errors
///
/// 查不了。
pub fn elevated() -> io::Result<bool> {
    let administrators = Sid::well_known(WinBuiltinAdministratorsSid)?;
    let mut member = 0;
    // SAFETY: 令牌传空，查的是当前线程（没有的话是进程）的令牌；SID 有效，这个接口只读它；`member` 是出参。
    let checked =
        unsafe { CheckTokenMembership(ptr::null_mut(), administrators.as_psid(), &mut member) };
    if checked == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(member != 0)
}

/// 拿 `runas` 起一个提升过的 `program`，参数是 `args`（照 Windows 命令行的规矩加引号），等它退出，交回它的退出码。
///
/// # Errors
///
/// 人取消了（[`Elevation::Cancelled`]）；起不来、等不了（[`Elevation::Failed`]）。
pub fn elevate(program: &Path, args: &[OsString]) -> Result<u32, Elevation> {
    let words: Vec<Vec<u16>> = args.iter().map(|arg| arg.encode_wide().collect()).collect();
    let mut parameters = quote::command_line(&words).map_err(Elevation::Failed)?;
    parameters.push(0);
    let verb = wide("runas");
    let file = wide(program);
    let mut info = SHELLEXECUTEINFOW {
        cbSize: u32::try_from(size_of::<SHELLEXECUTEINFOW>())
            .map_err(|error| Elevation::Failed(io::Error::other(error)))?,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI,
        lpVerb: verb.as_ptr(),
        lpFile: file.as_ptr(),
        lpParameters: parameters.as_ptr(),
        nShow: SW_HIDE,
        ..Default::default()
    };
    // SAFETY: `info` 填好了，里面的三个字符串以零结尾、活到这个函数结束；要了新进程的句柄，系统写进 `hProcess`。
    if unsafe { ShellExecuteExW(&mut info) } == 0 {
        let error = io::Error::last_os_error();
        let cancelled = error.raw_os_error() == i32::try_from(ERROR_CANCELLED).ok();
        return Err(if cancelled {
            Elevation::Cancelled
        } else {
            Elevation::Failed(error)
        });
    }
    if info.hProcess.is_null() {
        return Err(Elevation::Failed(io::Error::other(
            "the elevated process gave no handle",
        )));
    }
    let process = Handle(info.hProcess);
    // SAFETY: `process` 是系统交回的、有效的进程句柄；一直等到它退出。
    if unsafe { WaitForSingleObject(process.0, INFINITE) } == WAIT_FAILED {
        return Err(Elevation::Failed(io::Error::last_os_error()));
    }
    let mut code = 0;
    // SAFETY: 句柄有效；`code` 是出参。
    if unsafe { GetExitCodeProcess(process.0, &mut code) } == 0 {
        return Err(Elevation::Failed(io::Error::last_os_error()));
    }
    Ok(code)
}
