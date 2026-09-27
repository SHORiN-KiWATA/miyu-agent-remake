//! 调 Windows 的安全接口。每个 `unsafe` 块前面写着为什么安全。

use std::ffi::{OsStr, c_void};
use std::io;
use std::os::windows::io::AsRawHandle;
use std::ptr;

use tokio::net::windows::named_pipe::{NamedPipeClient, NamedPipeServer, ServerOptions};
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, LocalFree};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows_sys::Win32::Security::{
    GetTokenInformation, PSECURITY_DESCRIPTOR, PSID, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER,
    TokenUser,
};
use windows_sys::Win32::System::Pipes::GetNamedPipeServerProcessId;
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
};

/// 建管道 `name` 的一个实例，只对当前用户开放。`first` 为真时声明是第一个实例：这个名字已经有人占着，
/// 就建不成（拒绝访问）。
///
/// 要在 tokio 运行时里调：管道要登记到它上面。
///
/// # Errors
///
/// 取不到当前用户；安全描述符拼不出来；建不成。
pub fn create(name: &OsStr, first: bool) -> io::Result<NamedPipeServer> {
    let descriptor = descriptor(&crate::owner_only(&current_user()?))?;
    let mut attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: 0,
    };
    let mut options = ServerOptions::new();
    options.first_pipe_instance(first);
    // SAFETY: `attributes` 是一份填好的 SECURITY_ATTRIBUTES，里面的安全描述符在 `descriptor` 丢掉之前一直
    // 有效；建管道时系统照它设好访问控制，建完就不再用这个指针。
    unsafe {
        options.create_with_security_attributes_raw(name, (&raw mut attributes).cast::<c_void>())
    }
}

/// 当前用户的 SID，写成 `S-1-5-21-…` 这样的字。
///
/// # Errors
///
/// 取不到。
pub fn current_user() -> io::Result<String> {
    // SAFETY: 没有参数，给的是当前进程的伪句柄，不用关。
    let process = unsafe { GetCurrentProcess() };
    process_user(process)
}

/// 管道另一头（服务端）进程的用户 SID。
///
/// # Errors
///
/// 问不出另一头的进程编号；打不开那个进程（例如别人的、受保护的）；取不到它的用户。
pub fn server_user(pipe: &NamedPipeClient) -> io::Result<String> {
    let mut pid = 0u32;
    // SAFETY: 管道的句柄在借用期间一直有效；`pid` 是给系统写的出参。
    if unsafe { GetNamedPipeServerProcessId(pipe.as_raw_handle(), &mut pid) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: 只要查询信息的权限；拿到的句柄交给 `Handle`，丢掉时关。
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        return Err(io::Error::last_os_error());
    }
    let process = Handle(process);
    process_user(process.0)
}

/// 进程的用户 SID。
fn process_user(process: HANDLE) -> io::Result<String> {
    let mut token: HANDLE = ptr::null_mut();
    // SAFETY: `process` 是有效的进程句柄；`token` 是出参，拿到的句柄交给 `Handle`。
    if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let token = Handle(token);
    let mut size = 0u32;
    // SAFETY: 缓冲区传空、长度 0，只为问出要多大：这一次照例失败，`size` 里是要的大小。
    unsafe { GetTokenInformation(token.0, TokenUser, ptr::null_mut(), 0, &mut size) };
    if size == 0 {
        return Err(io::Error::last_os_error());
    }
    // 按 8 字节对齐：TOKEN_USER 里有指针。
    let mut buffer = vec![0u64; (size as usize).div_ceil(8)];
    // SAFETY: `buffer` 至少 `size` 个字节、按 8 字节对齐，够放 TOKEN_USER 和跟在它后面的 SID。
    let got = unsafe {
        GetTokenInformation(
            token.0,
            TokenUser,
            buffer.as_mut_ptr().cast(),
            size,
            &mut size,
        )
    };
    if got == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: 系统在 `buffer` 开头写好了一份 TOKEN_USER，它的 SID 指针指向 `buffer` 里面；`buffer` 活到
    // 这个函数结束，SID 在那之前就写成了字。
    let user = unsafe { &*buffer.as_ptr().cast::<TOKEN_USER>() };
    sid_text(user.User.Sid)
}

/// SID 写成字。
fn sid_text(sid: PSID) -> io::Result<String> {
    let mut text: *mut u16 = ptr::null_mut();
    // SAFETY: `sid` 是有效的 SID；`text` 是出参，系统用 LocalAlloc 分配，交给 `Local` 释放。
    if unsafe { ConvertSidToStringSidW(sid, &mut text) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let text = Local(text.cast());
    Ok(wide(text.0.cast::<u16>().cast_const()))
}

/// 照 SDDL 拼出安全描述符。
fn descriptor(sddl: &str) -> io::Result<Local> {
    let sddl: Vec<u16> = sddl.encode_utf16().chain([0]).collect();
    let mut descriptor: PSECURITY_DESCRIPTOR = ptr::null_mut();
    // SAFETY: `sddl` 以零结尾；`descriptor` 是出参，系统用 LocalAlloc 分配，交给 `Local` 释放。
    let made = unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            ptr::null_mut(),
        )
    };
    if made == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(Local(descriptor))
}

/// 系统给的、以零结尾的宽字符串。
fn wide(text: *const u16) -> String {
    let mut length = 0;
    // SAFETY: `text` 指向以零结尾的宽字符串，读到零为止，不越界。
    while unsafe { *text.add(length) } != 0 {
        length += 1;
    }
    // SAFETY: 上面数过，这 `length` 个都在字符串里面。
    String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(text, length) })
}

/// 打开的句柄，丢掉时关。
struct Handle(HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: 句柄是 OpenProcess、OpenProcessToken 给的，只在这里关一次。
        unsafe { CloseHandle(self.0) };
    }
}

/// 系统用 LocalAlloc 分配的一块，丢掉时释放。
struct Local(*mut c_void);

impl Drop for Local {
    fn drop(&mut self) {
        // SAFETY: 这块是系统用 LocalAlloc 分配的，只在这里释放一次。
        unsafe { LocalFree(self.0) };
    }
}

#[cfg(test)]
mod tests;
