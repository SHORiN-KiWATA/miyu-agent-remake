//! 沙盒用户的密码用 DPAPI 加密（`docs/blueprint/sandbox/windows.md`「装到哪了那份记录」）。
//!
//! 用机器范围：提升过的自己可能是另一个管理员账号（标准用户在 UAC 里输了别人的密码），用当前用户范围加密，核心那时
//! 以本人的身份解不开（2026-09-28 主会话审过时定）。机器范围谁在这台机器上都解得开，所以挡人的是记录文件的访问
//! 控制：只给本人和 SYSTEM。
//!
//! 这是放开 `unsafe` 的几个模块之一（施工 5-8）：每个 `unsafe` 块前面写着为什么安全。

#![allow(unsafe_code, reason = "调 Windows 的系统接口（施工 5-8）")]

use std::io;
use std::ptr;

use windows_sys::Win32::Security::Cryptography::{
    CRYPT_INTEGER_BLOB, CRYPTPROTECT_LOCAL_MACHINE, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData,
};

use super::winsys::Local;

/// 用机器范围的 DPAPI 加密 `data`：交回密文。
///
/// # Errors
///
/// 太长；系统加密不了。
pub(crate) fn protect(data: &[u8]) -> io::Result<Vec<u8>> {
    let input = CRYPT_INTEGER_BLOB {
        cbData: u32::try_from(data.len()).map_err(io::Error::other)?,
        pbData: data.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    // SAFETY: `input` 指向 `data`，调用期间有效，系统只读它；说明、熵、提示都不要，传空；不弹界面；`output` 是出参，
    // 系统用 LocalAlloc 分配，下面交给 `Local` 释放。
    let protected = unsafe {
        CryptProtectData(
            &input,
            ptr::null(),
            ptr::null(),
            ptr::null(),
            ptr::null(),
            CRYPTPROTECT_LOCAL_MACHINE | CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if protected == 0 {
        return Err(io::Error::last_os_error());
    }
    let owned = Local(output.pbData.cast());
    // SAFETY: 成功时 `output` 是系统分配的 `cbData` 个字节，在 `owned` 丢掉之前一直有效；这里抄一份出来。
    let bytes =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) }.to_vec();
    drop(owned);
    Ok(bytes)
}

#[cfg(test)]
mod tests;
