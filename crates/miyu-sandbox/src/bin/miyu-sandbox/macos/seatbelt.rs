//! 调系统的 `sandbox_init_with_parameters`，把配置装到自己身上（`docs/blueprint/sandbox/macos.md`「怎么走」第 3 条）。
//!
//! 整个 crate 只有这里（和 macOS 测试里调 `fcntl` 的那一处）用 `unsafe`：这两个函数在系统的库里，不在公开的头文件
//! 里，Rust 没有现成的安全封装；`sandbox-exec -D` 用的就是它。不经 `/usr/bin/sandbox-exec` 起命令：它自己出了错，印
//! 它自己的话、用它自己的退出码，核心分不出是没关进去，还是命令自己失败了（设计 `11-权限与沙盒.md` A8）。
//!
//! 装不上时，系统自己先在标准错误上印一句 `sandbox initialization failed: <原话>`，接着才是助手说的那一句。

#![expect(
    unsafe_code,
    reason = "调系统的 sandbox_init_with_parameters、sandbox_free_error，每一处写明为什么安全"
)]

use std::ffi::{CStr, CString, c_char, c_int};
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::ptr;

unsafe extern "C" {
    /// 照 `profile`（配置的字）把调它的进程关进沙盒，关上就收不回来。`flags` 写 0；`parameters` 是
    /// `名字, 值, 名字, 值, …`，最后一个空指针。成了交回 0；不成交回别的，`errorbuf` 指到系统写的原话，用完交给
    /// [`sandbox_free_error`]。
    fn sandbox_init_with_parameters(
        profile: *const c_char,
        flags: u64,
        parameters: *const *const c_char,
        errorbuf: *mut *mut c_char,
    ) -> c_int;

    /// 交还 [`sandbox_init_with_parameters`] 写的原话。
    fn sandbox_free_error(errorbuf: *mut c_char);
}

/// 把配置 `profile` 装到自己身上，`params` 是字里 `(param "…")` 说的路径。装上就收不回来，命令和它起的子进程都在
/// 里面。
///
/// # Errors
///
/// 装不上：交回系统的原话的第一行（配置写坏了的，后面几行是它的回溯）。字或路径里有 NUL 的（`resolve.rs` 已经拦过，
/// 走不到这里）也不装。
pub(crate) fn apply(profile: &str, params: &[(String, PathBuf)]) -> Result<(), String> {
    let text = CString::new(profile).map_err(|_| "the profile has a NUL byte".to_string())?;
    let mut owned = Vec::with_capacity(params.len() * 2);
    for (name, path) in params {
        let nul = |_| format!("path has a NUL byte: {path:?}");
        owned.push(CString::new(name.as_bytes()).map_err(nul)?);
        owned.push(CString::new(path.as_os_str().as_bytes()).map_err(nul)?);
    }
    let mut pointers: Vec<*const c_char> = owned.iter().map(|each| each.as_ptr()).collect();
    pointers.push(ptr::null());
    let mut error: *mut c_char = ptr::null_mut();
    // SAFETY: `text` 和 `owned` 里的每一串都以 NUL 结尾，活到这个函数结束，系统只在这次调用里读它们；`pointers` 最后
    // 一个是空指针，正是这个函数要的写法，也活到调用结束；`error` 是一个可写的指针变量，系统只往里写。
    let result = unsafe {
        sandbox_init_with_parameters(text.as_ptr(), 0, pointers.as_ptr(), &raw mut error)
    };
    if result == 0 {
        return Ok(());
    }
    if error.is_null() {
        return Err(format!("sandbox_init_with_parameters returned {result}"));
    }
    // SAFETY: 调用不成时，系统把 `error` 指到它分配的一串以 NUL 结尾的字，交还之前一直有效；这里只读，拷一份出来。
    let said = unsafe { CStr::from_ptr(error) }
        .to_string_lossy()
        .lines()
        .next()
        .unwrap_or_default()
        .to_string();
    // SAFETY: `error` 是系统刚交出来的、还没交还过的那一串，这里交还一次，之后不再用它。
    unsafe { sandbox_free_error(error) };
    Err(said)
}
