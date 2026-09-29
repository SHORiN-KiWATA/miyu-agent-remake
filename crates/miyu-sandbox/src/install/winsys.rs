//! 调 Windows 系统接口共用的几样（`docs/blueprint/sandbox/windows.md`「在哪」）：以零结尾的宽字符串，系统分配的内存和
//! 句柄丢掉时释放，SID 放在对齐的缓冲区里、写成字，内置组在这台机器上的名字，建好就只给本人和 SYSTEM 的文件。5-9 的
//! 受限令牌也用它们。
//!
//! 这是放开 `unsafe` 的几个模块之一（施工 5-8）：每个 `unsafe` 块前面写着为什么安全。

#![allow(unsafe_code, reason = "调 Windows 的系统接口（施工 5-8）")]

use std::ffi::{OsStr, c_void};
use std::fs::File;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::FromRawHandle;
use std::path::Path;
use std::ptr;

use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_INSUFFICIENT_BUFFER, ERROR_NONE_MAPPED, GENERIC_WRITE, HANDLE,
    INVALID_HANDLE_VALUE, LocalFree,
};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows_sys::Win32::Security::{
    CreateWellKnownSid, LookupAccountNameW, LookupAccountSidW, PSECURITY_DESCRIPTOR, PSID,
    SECURITY_ATTRIBUTES, SECURITY_MAX_SID_SIZE, SID_NAME_USE, WELL_KNOWN_SID_TYPE,
};
use windows_sys::Win32::Storage::FileSystem::{
    CREATE_NEW, CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_FLAG_OPEN_REPARSE_POINT,
};

/// 以零结尾的宽字符串：交给系统接口的名字、路径。
pub(crate) fn wide(text: impl AsRef<OsStr>) -> Vec<u16> {
    text.as_ref().encode_wide().chain([0]).collect()
}

/// 读系统写的、以零结尾的宽字符串。
///
/// # Safety
///
/// `text` 要指向一个以零结尾的宽字符串，读到零为止都有效。
pub(crate) unsafe fn from_wide(text: *const u16) -> String {
    let mut length = 0;
    // SAFETY: 调用的一方保证 `text` 以零结尾：读到零为止，不越界。
    while unsafe { *text.add(length) } != 0 {
        length += 1;
    }
    // SAFETY: 上面数过，这 `length` 个都在字符串里面。
    String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(text, length) })
}

/// 系统用 LocalAlloc 分配的一块，丢掉时释放。
pub(crate) struct Local(pub(crate) *mut c_void);

impl Drop for Local {
    fn drop(&mut self) {
        // SAFETY: 这块是系统用 LocalAlloc 分配的（或者是空的，空的释放不做事），只在这里释放一次。
        unsafe { LocalFree(self.0) };
    }
}

/// 打开的句柄，丢掉时关。
pub(crate) struct Handle(pub(crate) HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: 句柄是系统交回的、有效的，只在这里关一次。
        unsafe { CloseHandle(self.0) };
    }
}

/// 一个 SID，放在按 4 字节对齐的缓冲区里：SID 里有 32 位的子机构，照字节放可能没对齐。
pub(crate) struct Sid(Vec<u32>);

impl Sid {
    /// 内置的一个 SID，例如 Administrators、Users。
    ///
    /// # Errors
    ///
    /// 系统建不出来。
    pub(crate) fn well_known(kind: WELL_KNOWN_SID_TYPE) -> io::Result<Sid> {
        let mut size = SECURITY_MAX_SID_SIZE;
        let mut words = vec![0u32; (SECURITY_MAX_SID_SIZE as usize).div_ceil(4)];
        // SAFETY: `words` 有 `size` 个字节、按 4 字节对齐，够放最长的 SID；内置组的 SID 不要域的 SID，传空。
        let made = unsafe {
            CreateWellKnownSid(kind, ptr::null_mut(), words.as_mut_ptr().cast(), &mut size)
        };
        if made == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Sid(words))
    }

    /// 交给系统接口的指针：只给只读的接口用。
    pub(crate) fn as_psid(&self) -> PSID {
        self.0.as_ptr().cast_mut().cast()
    }

    /// 写成 `S-1-…` 的字。
    ///
    /// # Errors
    ///
    /// 系统写不出来。
    pub(crate) fn text(&self) -> io::Result<String> {
        let mut text: *mut u16 = ptr::null_mut();
        // SAFETY: SID 是有效的；`text` 是出参，系统用 LocalAlloc 分配，交给 `Local` 释放。
        if unsafe { ConvertSidToStringSidW(self.as_psid(), &mut text) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let text = Local(text.cast());
        // SAFETY: 成功时系统写的是以零结尾的宽字符串，活到 `text` 丢掉。
        Ok(unsafe { from_wide(text.0.cast()) })
    }

    /// 照名字查一个账号的 SID 和它是什么（用户、组……）：这台机器上没有这个名字的，是 `None`。本机的账号先于域里的查。
    ///
    /// # Errors
    ///
    /// 查不了。
    pub(crate) fn of_account(name: &str) -> io::Result<Option<(Sid, SID_NAME_USE)>> {
        let name = wide(name);
        let mut size = SECURITY_MAX_SID_SIZE;
        let mut words = vec![0u32; (SECURITY_MAX_SID_SIZE as usize).div_ceil(4)];
        let mut domain_length = 0u32;
        let mut kind: SID_NAME_USE = 0;
        // SAFETY: `name` 以零结尾；`words` 有 `size` 个字节、按 4 字节对齐，够放最长的 SID；域名的缓冲区传空、长度
        // 0，只为问出要多大：这一次照例因为域名放不下而失败。
        let found = unsafe {
            LookupAccountNameW(
                ptr::null(),
                name.as_ptr(),
                words.as_mut_ptr().cast(),
                &mut size,
                ptr::null_mut(),
                &mut domain_length,
                &mut kind,
            )
        };
        if found == 0 {
            let error = io::Error::last_os_error();
            match error
                .raw_os_error()
                .and_then(|code| u32::try_from(code).ok())
            {
                Some(ERROR_NONE_MAPPED) => return Ok(None),
                Some(ERROR_INSUFFICIENT_BUFFER) => {}
                _ => return Err(error),
            }
        }
        let mut domain = vec![0u16; domain_length.max(1) as usize];
        domain_length = u32::try_from(domain.len()).map_err(io::Error::other)?;
        size = SECURITY_MAX_SID_SIZE;
        // SAFETY: 同上；域名的缓冲区照上一次问出来的大小给，长度照实交过去，系统不会写过界。
        let found = unsafe {
            LookupAccountNameW(
                ptr::null(),
                name.as_ptr(),
                words.as_mut_ptr().cast(),
                &mut size,
                domain.as_mut_ptr(),
                &mut domain_length,
                &mut kind,
            )
        };
        if found == 0 {
            let error = io::Error::last_os_error();
            return match error
                .raw_os_error()
                .and_then(|code| u32::try_from(code).ok())
            {
                Some(ERROR_NONE_MAPPED) => Ok(None),
                _ => Err(error),
            };
        }
        Ok(Some((Sid(words), kind)))
    }

    /// 这台机器上的名字：内置组的名字是本地化的，中文系统上也照它找。
    ///
    /// # Errors
    ///
    /// 查不到。
    pub(crate) fn account_name(&self) -> io::Result<String> {
        let (mut name_length, mut domain_length) = (0u32, 0u32);
        let mut kind: SID_NAME_USE = 0;
        // SAFETY: 缓冲区传空、长度 0，只为问出要多大：这一次照例失败，两个长度里是要的大小（带结尾的零）。
        unsafe {
            LookupAccountSidW(
                ptr::null(),
                self.as_psid(),
                ptr::null_mut(),
                &mut name_length,
                ptr::null_mut(),
                &mut domain_length,
                &mut kind,
            )
        };
        if name_length == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut name = vec![0u16; name_length as usize];
        let mut domain = vec![0u16; domain_length.max(1) as usize];
        domain_length = u32::try_from(domain.len()).map_err(io::Error::other)?;
        // SAFETY: 两个缓冲区照上一次问出来的大小给，长度也照实交过去，系统不会写过界。
        let found = unsafe {
            LookupAccountSidW(
                ptr::null(),
                self.as_psid(),
                name.as_mut_ptr(),
                &mut name_length,
                domain.as_mut_ptr(),
                &mut domain_length,
                &mut kind,
            )
        };
        if found == 0 {
            return Err(io::Error::last_os_error());
        }
        name.truncate(name_length as usize);
        Ok(String::from_utf16_lossy(&name))
    }
}

/// 照访问控制的写法（SDDL）拼出安全描述符，丢掉时释放。
///
/// # Errors
///
/// 写法不对。
fn descriptor(sddl: &str) -> io::Result<Local> {
    let sddl = wide(sddl);
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

/// 只许新建地建 `path`，不跟重解析点；建的时候访问控制就是 `D:P(A;;FA;;;<sid>)(A;;FA;;;SY)`：受保护、不继承上一层，
/// 只有本人和 SYSTEM。`sid` 要核对过写法（`Owner::new`）：它原样拼进访问控制的写法里。
///
/// # Errors
///
/// 已经有了（连同链接）；建不了。
pub(crate) fn create_private(path: &Path, sid: &str) -> io::Result<File> {
    let descriptor = descriptor(&format!("D:P(A;;FA;;;{sid})(A;;FA;;;SY)"))?;
    let attributes = SECURITY_ATTRIBUTES {
        nLength: u32::try_from(size_of::<SECURITY_ATTRIBUTES>()).map_err(io::Error::other)?,
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: 0,
    };
    let path = wide(path);
    // SAFETY: `path` 以零结尾；`attributes` 里的安全描述符在 `descriptor` 丢掉之前一直有效，系统建文件时照它设好访问
    // 控制；只许新建（已经有了的，连同链接，都建不成），不跟重解析点；不共享，模板传空。
    let handle = unsafe {
        CreateFileW(
            path.as_ptr(),
            GENERIC_WRITE,
            0,
            &attributes,
            CREATE_NEW,
            FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
            ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `handle` 是刚打开的文件句柄，只交给这一个 `File`，由它关。
    Ok(unsafe { File::from_raw_handle(handle) })
}

/// 读出一个文件的访问控制，写成 SDDL：测试看建出来的对不对。
#[cfg(test)]
pub(crate) fn dacl_of(path: &Path) -> io::Result<String> {
    use windows_sys::Win32::Security::Authorization::{GetNamedSecurityInfoW, SE_FILE_OBJECT};
    use windows_sys::Win32::Security::DACL_SECURITY_INFORMATION;

    let path = wide(path);
    let mut descriptor: PSECURITY_DESCRIPTOR = ptr::null_mut();
    // SAFETY: `path` 以零结尾；只要访问控制，别的出参传空；`descriptor` 由系统用 LocalAlloc 分配，交给 `Local` 释放。
    let status = unsafe {
        GetNamedSecurityInfoW(
            path.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            &mut descriptor,
        )
    };
    if status != 0 {
        return Err(io::Error::from_raw_os_error(
            i32::try_from(status).unwrap_or(i32::MAX),
        ));
    }
    dacl_text(&Local(descriptor))
}

/// 一段访问控制的写法照系统的规矩重写一遍：同一份访问控制，写法可能不一样（例如内置管理员的 SID 写成 `LA`），比之前都
/// 过一遍系统，比的才是访问控制本身。
#[cfg(test)]
pub(crate) fn canonical_dacl(sddl: &str) -> io::Result<String> {
    dacl_text(&descriptor(sddl)?)
}

/// 安全描述符里的访问控制写成 SDDL。
#[cfg(test)]
fn dacl_text(descriptor: &Local) -> io::Result<String> {
    use windows_sys::Win32::Security::Authorization::ConvertSecurityDescriptorToStringSecurityDescriptorW;
    use windows_sys::Win32::Security::DACL_SECURITY_INFORMATION;

    let mut text: *mut u16 = ptr::null_mut();
    // SAFETY: `descriptor` 是有效的安全描述符；`text` 是出参，系统用 LocalAlloc 分配，交给 `Local` 释放。
    let written = unsafe {
        ConvertSecurityDescriptorToStringSecurityDescriptorW(
            descriptor.0,
            SDDL_REVISION_1,
            DACL_SECURITY_INFORMATION,
            &mut text,
            ptr::null_mut(),
        )
    };
    if written == 0 {
        return Err(io::Error::last_os_error());
    }
    let text = Local(text.cast());
    // SAFETY: 成功时系统写的是以零结尾的宽字符串，活到 `text` 丢掉。
    Ok(unsafe { from_wide(text.0.cast()) })
}
