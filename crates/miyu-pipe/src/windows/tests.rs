use tokio::net::windows::named_pipe::ClientOptions;
use windows_sys::Win32::Security::Authorization::{
    ConvertSecurityDescriptorToStringSecurityDescriptorW, ConvertStringSidToSidW, GetSecurityInfo,
    SE_KERNEL_OBJECT,
};
use windows_sys::Win32::Security::DACL_SECURITY_INFORMATION;

use super::*;

/// 这个测试用的管道名：进程号加标签，几个测试不撞。
fn name(tag: &str) -> String {
    format!(r"\\.\pipe\miyu-pipe-test-{}-{tag}", std::process::id())
}

/// 从头这一边读回管道的访问控制列表，写成 SDDL。
fn dacl(pipe: &NamedPipeClient) -> String {
    let mut descriptor: PSECURITY_DESCRIPTOR = ptr::null_mut();
    // SAFETY: 句柄在借用期间有效；只要访问控制列表，别的出参传空；安全描述符交给 `Local` 释放。
    let status = unsafe {
        GetSecurityInfo(
            pipe.as_raw_handle(),
            SE_KERNEL_OBJECT,
            DACL_SECURITY_INFORMATION,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            &mut descriptor,
        )
    };
    assert_eq!(status, 0, "读得到访问控制");
    let descriptor = Local(descriptor);
    let mut text: *mut u16 = ptr::null_mut();
    // SAFETY: 安全描述符有效；`text` 是出参，交给 `Local` 释放。
    let made = unsafe {
        ConvertSecurityDescriptorToStringSecurityDescriptorW(
            descriptor.0,
            SDDL_REVISION_1,
            DACL_SECURITY_INFORMATION,
            &mut text,
            ptr::null_mut(),
        )
    };
    assert_ne!(made, 0, "写得成 SDDL");
    let text = Local(text.cast());
    wide(text.0.cast::<u16>().cast_const())
}

/// SDDL 里写的用户写成 `S-1-…`：系统写 SDDL 时，常见的账号会换成简写，例如内置的管理员账号写成 `LA`。
fn full_sid(trustee: &str) -> String {
    let trustee: Vec<u16> = trustee.encode_utf16().chain([0]).collect();
    let mut sid: PSID = ptr::null_mut();
    // SAFETY: `trustee` 以零结尾；`sid` 是出参，系统用 LocalAlloc 分配，交给 `Local` 释放。
    let made = unsafe { ConvertStringSidToSidW(trustee.as_ptr(), &mut sid) };
    assert_ne!(made, 0, "认得出这个用户");
    let sid = Local(sid);
    sid_text(sid.0).expect("写得成字")
}

#[tokio::test]
async fn the_pipe_is_open_to_its_owner_only() {
    let name = name("owner");
    let _server = create(OsStr::new(&name), true).expect("建得成");
    let client = ClientOptions::new().open(&name).expect("自己连得上");
    let me = current_user().expect("知道自己是谁");
    let sddl = dacl(&client);
    // 例如 `D:P(A;;FA;;;S-1-5-21-…)`：受保护、不继承上一层的，只有一条，允许，给的是自己。
    let ace = sddl
        .strip_prefix("D:P(")
        .and_then(|rest| rest.strip_suffix(')'))
        .unwrap_or_else(|| panic!("受保护、只有一条：{sddl}"));
    assert!(!ace.contains('('), "只有一条：{sddl}");
    let fields: Vec<&str> = ace.split(';').collect();
    assert_eq!(fields.len(), 6, "{sddl}");
    assert_eq!(fields[0], "A", "这一条是允许：{sddl}");
    assert_eq!(full_sid(fields[5]), me, "给的是自己：{sddl}");
}

#[tokio::test]
async fn a_name_has_only_one_first_instance() {
    let name = name("first");
    let _first = create(OsStr::new(&name), true).expect("建得成");
    let error = create(OsStr::new(&name), true).expect_err("第二个「第一个实例」建不成");
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    create(OsStr::new(&name), false).expect("后面的实例建得成");
}

#[tokio::test]
async fn a_head_sees_its_own_core_at_the_other_end() {
    let name = name("server");
    let _server = create(OsStr::new(&name), true).expect("建得成");
    let client = ClientOptions::new().open(&name).expect("连得上");
    assert_eq!(
        server_user(&client).expect("问得出另一头"),
        current_user().expect("知道自己是谁")
    );
}

#[test]
fn the_current_user_is_a_sid() {
    let me = current_user().expect("知道自己是谁");
    assert!(me.starts_with("S-1-"), "{me}");
}
