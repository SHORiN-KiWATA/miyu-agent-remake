//! 建、重置、藏、删沙盒用户（`docs/blueprint/sandbox/windows.md`「沙盒用户」、「怎么走」第 4、5 条）。每一样都幂等：
//! 已经是那样的不算错。名字由调用的一方给：装的时候是 `miyu-sandbox`，虚拟机上的测试用别的名字。
//!
//! 这是放开 `unsafe` 的几个模块之一（施工 5-8）：每个 `unsafe` 块前面写着为什么安全。

#![allow(unsafe_code, reason = "调 Windows 的系统接口（施工 5-8）")]

use std::ffi::c_void;
use std::io;
use std::ptr;

use windows_sys::Win32::Foundation::{
    ERROR_FILE_NOT_FOUND, ERROR_MEMBER_IN_ALIAS, ERROR_PATH_NOT_FOUND,
};
// 账号管理接口的这几个码名字是大小写混着的，放进 `match` 里当模式会被当成变量名的写法挨说：换个全大写的名字。
use windows_sys::Win32::NetworkManagement::NetManagement::{
    LG_INCLUDE_INDIRECT, LOCALGROUP_MEMBERS_INFO_3, LOCALGROUP_USERS_INFO_0, MAX_PREFERRED_LENGTH,
    NERR_Success as NERR_SUCCESS, NERR_UserExists as NERR_USER_EXISTS,
    NERR_UserNotFound as NERR_USER_NOT_FOUND, NetApiBufferFree, NetLocalGroupAddMembers,
    NetUserAdd, NetUserDel, NetUserGetLocalGroups, NetUserSetInfo, UF_DONT_EXPIRE_PASSWD,
    UF_NORMAL_ACCOUNT, UF_PASSWD_CANT_CHANGE, UF_SCRIPT, USER_INFO_1, USER_INFO_1003,
    USER_INFO_1008, USER_PRIV_USER,
};
use windows_sys::Win32::Security::{SidTypeUser, WinBuiltinAdministratorsSid, WinBuiltinUsersSid};
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_LOCAL_MACHINE, KEY_SET_VALUE, REG_DWORD, REG_OPTION_NON_VOLATILE, RegCloseKey,
    RegCreateKeyExW, RegDeleteKeyValueW, RegSetValueExW,
};
use windows_sys::Win32::UI::Shell::DeleteProfileW;

use super::InstallError;
use super::winsys::{Sid, from_wide, wide};

/// 沙盒用户的标志：普通用户、启用（不带停用那一位）、密码不过期、自己改不了密码。`UF_SCRIPT` 是接口要求必带的。
pub(crate) const FLAGS: u32 =
    UF_SCRIPT | UF_NORMAL_ACCOUNT | UF_DONT_EXPIRE_PASSWD | UF_PASSWD_CANT_CHANGE;

/// 账号的说明，在「本地用户和组」里看得到。
pub(crate) const COMMENT: &str = "Miyu sandbox";

/// 登录界面上藏哪些账号：这把钥匙下，名字是账号、值是 0 的，不显示。
pub(crate) const USER_LIST: &str =
    r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon\SpecialAccounts\UserList";

/// 系统接口交回的错误码说成一句：账号管理接口自己的码（`NERR_*`，2100 到 2999）系统的说法里查不到，照编号说；别的
/// 是系统的错误码，照系统的原话（中文系统上是中文）。
fn status_text(status: u32) -> String {
    if (2100..=2999).contains(&status) {
        format!("network management error {status}")
    } else {
        i32::try_from(status).map_or_else(
            |_| format!("error {status}"),
            |code| io::Error::from_raw_os_error(code).to_string(),
        )
    }
}

/// 已有的 `name` 是不是管理员（连经别的组间接在 `Administrators` 里的）：是的，不动它（`check account`）。没有这个
/// 账号不算错。
///
/// # Errors
///
/// 是管理员（[`InstallError::Administrator`]）；查不了。
pub(crate) fn check(name: &str) -> Result<(), InstallError> {
    let failed = |detail: &dyn std::fmt::Display| InstallError::failed("check account", detail);
    let administrators = Sid::well_known(WinBuiltinAdministratorsSid)
        .and_then(|sid| sid.account_name())
        .map_err(|error| failed(&error))?;
    let groups = match local_groups(name) {
        Ok(groups) => groups,
        Err(NERR_USER_NOT_FOUND) => return Ok(()),
        Err(status) => return Err(failed(&status_text(status))),
    };
    if groups
        .iter()
        .any(|group| group.to_lowercase() == administrators.to_lowercase())
    {
        return Err(InstallError::Administrator);
    }
    Ok(())
}

/// `name` 在哪些本机组里，连间接的：交回组名；查不了的交回账号管理接口的错误码。
pub(crate) fn local_groups(name: &str) -> Result<Vec<String>, u32> {
    let name = wide(name);
    let mut buffer: *mut u8 = ptr::null_mut();
    let (mut read, mut total) = (0u32, 0u32);
    // SAFETY: `name` 以零结尾；`buffer` 是出参，系统分配，交给 `NetBuffer` 释放；`MAX_PREFERRED_LENGTH` 让它一次
    // 交回全部。
    let status = unsafe {
        NetUserGetLocalGroups(
            ptr::null(),
            name.as_ptr(),
            0,
            LG_INCLUDE_INDIRECT,
            &mut buffer,
            MAX_PREFERRED_LENGTH,
            &mut read,
            &mut total,
        )
    };
    let buffer = NetBuffer(buffer.cast());
    if status != NERR_SUCCESS {
        return Err(status);
    }
    if buffer.0.is_null() || read == 0 {
        return Ok(Vec::new());
    }
    // SAFETY: 成功时 `buffer` 里是 `read` 个 LOCALGROUP_USERS_INFO_0，一直有效到 `buffer` 丢掉。
    let entries = unsafe {
        std::slice::from_raw_parts(buffer.0.cast::<LOCALGROUP_USERS_INFO_0>(), read as usize)
    };
    Ok(entries
        .iter()
        // SAFETY: 每一项的名字是系统写的、以零结尾的宽字符串，在 `buffer` 里面。
        .map(|entry| unsafe { from_wide(entry.lgrui0_name) })
        .collect())
}

/// 建 `name`，密码 `password`，标志 [`FLAGS`]，说明 [`COMMENT`]（`create user`）。已经有了的：把密码重置成
/// `password`（`reset password`），再把标志设回 [`FLAGS`]（`set flags`），被停用了的也就启用了。
///
/// # Errors
///
/// 哪一步没成。
pub(crate) fn ensure(name: &str, password: &str) -> Result<(), InstallError> {
    let mut wide_name = wide(name);
    let mut wide_password = wide(password);
    let mut comment = wide(COMMENT);
    let info = USER_INFO_1 {
        usri1_name: wide_name.as_mut_ptr(),
        usri1_password: wide_password.as_mut_ptr(),
        usri1_password_age: 0,
        usri1_priv: USER_PRIV_USER,
        usri1_home_dir: ptr::null_mut(),
        usri1_comment: comment.as_mut_ptr(),
        usri1_flags: FLAGS,
        usri1_script_path: ptr::null_mut(),
    };
    let mut bad = 0u32;
    // SAFETY: 第 1 级收的就是 USER_INFO_1；里面的三个字符串以零结尾、活到调用结束，系统只读它们；`bad` 是出参。
    let status = unsafe { NetUserAdd(ptr::null(), 1, (&raw const info).cast(), &mut bad) };
    // 明文的密码用完就抹掉，不在内存里多留。
    wide_password.fill(0);
    match status {
        NERR_SUCCESS => Ok(()),
        NERR_USER_EXISTS => {
            reset_password(name, password)?;
            set_flags(name, FLAGS)
        }
        other => Err(InstallError::failed("create user", status_text(other))),
    }
}

/// 把 `name` 的密码重置成 `password`（`reset password`）。
fn reset_password(name: &str, password: &str) -> Result<(), InstallError> {
    let wide_name = wide(name);
    let mut wide_password = wide(password);
    let info = USER_INFO_1003 {
        usri1003_password: wide_password.as_mut_ptr(),
    };
    let mut bad = 0u32;
    // SAFETY: 第 1003 级收的就是 USER_INFO_1003；名字、密码以零结尾、活到调用结束；`bad` 是出参。
    let status = unsafe {
        NetUserSetInfo(
            ptr::null(),
            wide_name.as_ptr(),
            1003,
            (&raw const info).cast(),
            &mut bad,
        )
    };
    wide_password.fill(0);
    match status {
        NERR_SUCCESS => Ok(()),
        other => Err(InstallError::failed("reset password", status_text(other))),
    }
}

/// 把 `name` 的标志设成 `flags`（`set flags`）。
///
/// # Errors
///
/// 设不了。
pub(crate) fn set_flags(name: &str, flags: u32) -> Result<(), InstallError> {
    let wide_name = wide(name);
    let info = USER_INFO_1008 {
        usri1008_flags: flags,
    };
    let mut bad = 0u32;
    // SAFETY: 第 1008 级收的就是 USER_INFO_1008；名字以零结尾、活到调用结束；`bad` 是出参。
    let status = unsafe {
        NetUserSetInfo(
            ptr::null(),
            wide_name.as_ptr(),
            1008,
            (&raw const info).cast(),
            &mut bad,
        )
    };
    match status {
        NERR_SUCCESS => Ok(()),
        other => Err(InstallError::failed("set flags", status_text(other))),
    }
}

/// 把 `name` 加进 `Users` 组（`join Users`）：组名照 SID `S-1-5-32-545` 查，中文系统上也对；已经在里面不算错。
///
/// # Errors
///
/// 查不到组名；加不进去。
pub(crate) fn join_users(name: &str) -> Result<(), InstallError> {
    let failed = |detail: &dyn std::fmt::Display| InstallError::failed("join Users", detail);
    let users = Sid::well_known(WinBuiltinUsersSid)
        .and_then(|sid| sid.account_name())
        .map_err(|error| failed(&error))?;
    let group = wide(&users);
    let mut member_name = wide(name);
    let member = LOCALGROUP_MEMBERS_INFO_3 {
        lgrmi3_domainandname: member_name.as_mut_ptr(),
    };
    // SAFETY: 第 3 级收的就是 LOCALGROUP_MEMBERS_INFO_3，一项；组名、成员名以零结尾、活到调用结束。
    let status = unsafe {
        NetLocalGroupAddMembers(
            ptr::null(),
            group.as_ptr(),
            3,
            (&raw const member).cast(),
            1,
        )
    };
    match status {
        NERR_SUCCESS | ERROR_MEMBER_IN_ALIAS => Ok(()),
        other => Err(failed(&status_text(other))),
    }
}

/// 在登录界面上藏起 `name`（`hide user`）：[`USER_LIST`] 下写一个名字是它、值是 0 的 `DWORD`。
///
/// # Errors
///
/// 打不开、写不进注册表。
pub(crate) fn hide(name: &str) -> Result<(), InstallError> {
    let failed = |status: u32| InstallError::failed("hide user", status_text(status));
    let path = wide(USER_LIST);
    let mut key: HKEY = ptr::null_mut();
    // SAFETY: 路径以零结尾；只要写值的权限；没有就建、有就打开；拿到的钥匙交给 `Key`，丢掉时关。
    let status = unsafe {
        RegCreateKeyExW(
            HKEY_LOCAL_MACHINE,
            path.as_ptr(),
            0,
            ptr::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            ptr::null(),
            &mut key,
            ptr::null_mut(),
        )
    };
    if status != 0 {
        return Err(failed(status));
    }
    let key = Key(key);
    let value = wide(name);
    let hidden: u32 = 0;
    let size = u32::try_from(size_of_val(&hidden)).map_err(|_| failed(0))?;
    // SAFETY: 钥匙有效；值名以零结尾；数据是一个 `u32`，长度照实给。
    let status = unsafe {
        RegSetValueExW(
            key.0,
            value.as_ptr(),
            0,
            REG_DWORD,
            (&raw const hidden).cast(),
            size,
        )
    };
    if status != 0 {
        return Err(failed(status));
    }
    Ok(())
}

/// 取消隐藏 `name`（`unhide user`）：删掉 [`USER_LIST`] 下它那个值。本来就没有（连钥匙都没有）不算错。
///
/// # Errors
///
/// 删不了。
pub(crate) fn unhide(name: &str) -> Result<(), InstallError> {
    let path = wide(USER_LIST);
    let value = wide(name);
    // SAFETY: 路径、值名以零结尾。
    let status = unsafe { RegDeleteKeyValueW(HKEY_LOCAL_MACHINE, path.as_ptr(), value.as_ptr()) };
    match status {
        0 | ERROR_FILE_NOT_FOUND => Ok(()),
        other => Err(InstallError::failed("unhide user", status_text(other))),
    }
}

/// `name` 的 SID，写成字（`look up user`）：没有这个账号是 `None`。有这个名字、却不是用户的（例如一个组），算错。
///
/// # Errors
///
/// 查不了；这个名字不是用户。
pub(crate) fn sid(name: &str) -> Result<Option<String>, InstallError> {
    let failed = |detail: &dyn std::fmt::Display| InstallError::failed("look up user", detail);
    match Sid::of_account(name).map_err(|error| failed(&error))? {
        None => Ok(None),
        Some((sid, kind)) if kind == SidTypeUser => {
            sid.text().map(Some).map_err(|error| failed(&error))
        }
        Some(_) => Err(failed(&format!("{name} is not a user account"))),
    }
}

/// 删掉 SID 是 `sid` 的用户的 profile（`delete profile`）：没有 profile 不算错。
///
/// # Errors
///
/// 删不了，例如这个用户还有进程在跑（5-9 起 `remove` 先停掉它们）。
pub(crate) fn delete_profile(sid: &str) -> Result<(), InstallError> {
    let sid = wide(sid);
    // SAFETY: SID 以零结尾；profile 的位置、机器名传空：照 SID 找，本机。
    if unsafe { DeleteProfileW(sid.as_ptr(), ptr::null(), ptr::null()) } != 0 {
        return Ok(());
    }
    let error = io::Error::last_os_error();
    match error
        .raw_os_error()
        .and_then(|code| u32::try_from(code).ok())
    {
        Some(ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND) => Ok(()),
        _ => Err(InstallError::failed("delete profile", error)),
    }
}

/// 删掉用户 `name`（`delete user`）：本来就没有不算错。
///
/// # Errors
///
/// 删不了。
pub(crate) fn delete(name: &str) -> Result<(), InstallError> {
    let name = wide(name);
    // SAFETY: 名字以零结尾。
    let status = unsafe { NetUserDel(ptr::null(), name.as_ptr()) };
    match status {
        NERR_SUCCESS | NERR_USER_NOT_FOUND => Ok(()),
        other => Err(InstallError::failed("delete user", status_text(other))),
    }
}

/// 账号管理接口分配的一块，丢掉时释放。
struct NetBuffer(*mut c_void);

impl Drop for NetBuffer {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: 这块是账号管理接口分配的，只在这里释放一次。
            unsafe { NetApiBufferFree(self.0) };
        }
    }
}

/// 打开的注册表钥匙，丢掉时关。
struct Key(HKEY);

impl Drop for Key {
    fn drop(&mut self) {
        // SAFETY: 钥匙是 RegCreateKeyExW 交回的、有效的，只在这里关一次。
        unsafe { RegCloseKey(self.0) };
    }
}

#[cfg(test)]
mod tests;
