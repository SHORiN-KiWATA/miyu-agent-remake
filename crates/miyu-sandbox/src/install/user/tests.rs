//! 建、藏、删沙盒用户（`docs/blueprint/sandbox/windows.md`「沙盒用户」、「怎么走」第 4、5 条）。
//!
//! 会真的在这台机器上建用户：全都标 `#[ignore]`，只在测试用的 Windows 虚拟机上以管理员身份跑
//! （`cargo test -p miyu-sandbox --lib -- --ignored`）。用的是单独的名字 [`TEST_USER`]，不碰真的 `miyu-sandbox`；
//! 每条测试开头、结尾都把它撤干净，上一次没跑完留下的也清得掉。

use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::NetworkManagement::NetManagement::{NetUserGetInfo, UF_ACCOUNTDISABLE};
use windows_sys::Win32::Security::{LOGON32_LOGON_NETWORK, LOGON32_PROVIDER_DEFAULT, LogonUserW};
use windows_sys::Win32::System::Registry::{RRF_RT_REG_DWORD, RegGetValueW};

use super::*;
use crate::install::winsys::Handle;

/// 测试用的账号名：和 `miyu-sandbox` 分开，也不超过 20 个字符。
const TEST_USER: &str = "miyu-sbx-test";

/// 测试看一个账号的样子。
struct State {
    privilege: u32,
    flags: u32,
    comment: String,
    in_users: bool,
    in_administrators: bool,
    hidden: bool,
}

/// 查 `name` 的级别、标志、说明、在哪些组里、藏没藏。
fn inspect(name: &str) -> io::Result<State> {
    let wide_name = wide(name);
    let mut buffer: *mut u8 = ptr::null_mut();
    // SAFETY: 名字以零结尾；`buffer` 是出参，系统分配，交给 `NetBuffer` 释放。
    let status = unsafe { NetUserGetInfo(ptr::null(), wide_name.as_ptr(), 1, &mut buffer) };
    let buffer = NetBuffer(buffer.cast());
    if status != NERR_SUCCESS {
        return Err(io::Error::other(status_text(status)));
    }
    // SAFETY: 成功时 `buffer` 里是一个 USER_INFO_1，一直有效到 `buffer` 丢掉。
    let info = unsafe { &*buffer.0.cast::<USER_INFO_1>() };
    let comment = match info.usri1_comment.is_null() {
        true => String::new(),
        // SAFETY: 说明是系统写的、以零结尾的宽字符串，在 `buffer` 里面。
        false => unsafe { from_wide(info.usri1_comment) },
    };
    let groups = local_groups(name).map_err(|status| io::Error::other(status_text(status)))?;
    let users = Sid::well_known(WinBuiltinUsersSid)?.account_name()?;
    let administrators = Sid::well_known(WinBuiltinAdministratorsSid)?.account_name()?;
    Ok(State {
        privilege: info.usri1_priv,
        flags: info.usri1_flags,
        comment,
        in_users: groups.contains(&users),
        in_administrators: groups.contains(&administrators),
        hidden: inspect_hidden(name),
    })
}

/// [`USER_LIST`] 下有没有 `name`、值是不是 0。
fn inspect_hidden(name: &str) -> bool {
    let path = wide(USER_LIST);
    let value = wide(name);
    let mut data = u32::MAX;
    let mut size = 4u32;
    // SAFETY: 路径、值名以零结尾；只收 `DWORD`，数据写进 `data`，长度照实给。
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            path.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_DWORD,
            ptr::null_mut(),
            (&raw mut data).cast(),
            &mut size,
        )
    };
    status == 0 && data == 0
}

/// 照 `password` 能不能以 `name` 的身份登录（网络登录：不加载 profile，登完就关）。
fn logs_on(name: &str, password: &str) -> bool {
    let (name, domain, password) = (wide(name), wide("."), wide(password));
    let mut token: HANDLE = ptr::null_mut();
    // SAFETY: 三个字符串以零结尾；`token` 是出参，成功时交给 `Handle` 关。
    let ok = unsafe {
        LogonUserW(
            name.as_ptr(),
            domain.as_ptr(),
            password.as_ptr(),
            LOGON32_LOGON_NETWORK,
            LOGON32_PROVIDER_DEFAULT,
            &mut token,
        )
    };
    if ok == 0 {
        return false;
    }
    drop(Handle(token));
    true
}

/// 把 `name` 加进 `Administrators`：测试「是管理员的不动它」。
fn add_to_administrators(name: &str) -> io::Result<()> {
    let administrators = Sid::well_known(WinBuiltinAdministratorsSid)?.account_name()?;
    let group = wide(&administrators);
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
        other => Err(io::Error::other(status_text(other))),
    }
}

/// 把测试用的账号撤干净：删 profile、删用户、取消隐藏，本来没有的不算错。
fn clean() {
    if let Some(sid) = sid(TEST_USER).expect("查得了") {
        delete_profile(&sid).expect("删得了 profile");
    }
    delete(TEST_USER).expect("删得了用户");
    unhide(TEST_USER).expect("取消得了隐藏");
}

#[test]
#[ignore = "会真的建用户：只在测试用的 Windows 虚拟机上以管理员身份跑"]
fn a_user_is_made_hidden_plain_and_can_log_on_with_the_password() {
    clean();
    ensure(TEST_USER, "Aa0!first-password-for-test-01").expect("建得了");
    join_users(TEST_USER).expect("加得进 Users");
    hide(TEST_USER).expect("藏得了");
    let state = inspect(TEST_USER).expect("查得到");
    assert_eq!(state.privilege, USER_PRIV_USER, "普通用户");
    assert_eq!(state.flags & UF_ACCOUNTDISABLE, 0, "启用着");
    assert_ne!(state.flags & UF_DONT_EXPIRE_PASSWD, 0, "密码不过期");
    assert_ne!(state.flags & UF_PASSWD_CANT_CHANGE, 0, "自己改不了密码");
    assert_eq!(state.comment, COMMENT);
    assert!(state.in_users, "在 Users 组里");
    assert!(!state.in_administrators, "不是管理员");
    assert!(state.hidden, "登录界面上藏着");
    assert!(
        logs_on(TEST_USER, "Aa0!first-password-for-test-01"),
        "照这个密码登得上"
    );
    assert!(
        sid(TEST_USER)
            .expect("查得了")
            .is_some_and(|sid| sid.starts_with("S-1-5-21-"))
    );
    check(TEST_USER).expect("不是管理员，照常装");
    clean();
}

#[test]
#[ignore = "会真的建用户：只在测试用的 Windows 虚拟机上以管理员身份跑"]
fn making_it_again_resets_the_password_and_enables_it() {
    clean();
    ensure(TEST_USER, "Aa0!first-password-for-test-01").expect("建得了");
    set_flags(TEST_USER, FLAGS | UF_ACCOUNTDISABLE).expect("停用得了");
    ensure(TEST_USER, "Bb1#second-password-for-test2").expect("再建也成");
    let state = inspect(TEST_USER).expect("查得到");
    assert_eq!(state.flags & UF_ACCOUNTDISABLE, 0, "又启用了");
    assert!(
        !logs_on(TEST_USER, "Aa0!first-password-for-test-01"),
        "旧密码不能用了"
    );
    assert!(
        logs_on(TEST_USER, "Bb1#second-password-for-test2"),
        "新密码能用"
    );
    join_users(TEST_USER).expect("加得进 Users");
    join_users(TEST_USER).expect("已经在里面不算错");
    hide(TEST_USER).expect("藏得了");
    hide(TEST_USER).expect("再藏也成");
    clean();
}

#[test]
#[ignore = "会真的建用户：只在测试用的 Windows 虚拟机上以管理员身份跑"]
fn an_administrator_by_that_name_is_left_alone() {
    clean();
    ensure(TEST_USER, "Aa0!first-password-for-test-01").expect("建得了");
    add_to_administrators(TEST_USER).expect("加得进 Administrators");
    assert_eq!(check(TEST_USER), Err(InstallError::Administrator));
    clean();
}

#[test]
#[ignore = "会真的建用户：只在测试用的 Windows 虚拟机上以管理员身份跑"]
fn removing_what_is_not_there_is_fine() {
    clean();
    assert_eq!(sid(TEST_USER).expect("查得了"), None, "没有这个用户");
    check(TEST_USER).expect("没有的不是管理员");
    delete(TEST_USER).expect("没有的删了不算错");
    unhide(TEST_USER).expect("没藏的取消隐藏不算错");
    assert!(!inspect_hidden(TEST_USER), "注册表里没留下");
}

#[test]
#[ignore = "会真的建用户：只在测试用的 Windows 虚拟机上以管理员身份跑"]
fn a_group_by_that_name_is_not_taken_for_the_user() {
    // 名字对得上的是一个组，不是用户：不当它是沙盒用户。
    assert!(sid("Users").is_err());
}
