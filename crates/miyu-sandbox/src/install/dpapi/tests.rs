//! 密码用机器范围的 DPAPI 加密（`docs/blueprint/sandbox/windows.md`「装到哪了那份记录」）：解得回来，密文里没有明文。

use windows_sys::Win32::Security::Cryptography::CryptUnprotectData;

use super::*;

/// 解开：5-9 登录沙盒用户时照这样做，这一步只有测试用。
fn unprotect(blob: &[u8]) -> io::Result<Vec<u8>> {
    let input = CRYPT_INTEGER_BLOB {
        cbData: u32::try_from(blob.len()).map_err(io::Error::other)?,
        pbData: blob.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    // SAFETY: `input` 指向 `blob`，调用期间有效，系统只读它；说明、熵、提示都不要，传空；`output` 是出参，系统用
    // LocalAlloc 分配，下面交给 `Local` 释放。
    let opened = unsafe {
        CryptUnprotectData(
            &input,
            ptr::null_mut(),
            ptr::null(),
            ptr::null(),
            ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if opened == 0 {
        return Err(io::Error::last_os_error());
    }
    let owned = Local(output.pbData.cast());
    // SAFETY: 成功时 `output` 是系统分配的 `cbData` 个字节，在 `owned` 丢掉之前一直有效。
    let bytes =
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) }.to_vec();
    drop(owned);
    Ok(bytes)
}

#[test]
fn what_is_protected_comes_back() {
    let secret = b"Aa0!correct horse battery staple";
    let blob = protect(secret).expect("加得了密");
    assert_eq!(unprotect(&blob).expect("解得开"), secret);
}

#[test]
fn the_blob_does_not_carry_the_secret_in_the_clear() {
    let secret = b"Aa0!correct horse battery staple";
    let blob = protect(secret).expect("加得了密");
    assert!(
        !blob.windows(secret.len()).any(|window| window == secret),
        "密文里有明文"
    );
    // UTF-16 的写法也不许有：有的接口会把它当宽字符存。
    let wide: Vec<u8> = secret.iter().flat_map(|b| [*b, 0]).collect();
    assert!(!blob.windows(wide.len()).any(|window| window == wide));
}

#[test]
fn every_protection_is_different() {
    // 每次加密都带新的随机盐：同一个密码两份密文不一样，看不出是不是同一个。
    let secret = b"same";
    assert_ne!(
        protect(secret).expect("加得了密"),
        protect(secret).expect("加得了密")
    );
}

#[test]
fn the_blob_is_machine_scoped() {
    // 提升过的自己可能是另一个管理员账号，核心以本人的身份也得解得开：要是机器范围的。密文的头上记着加密时的标志：
    // 版本 4 字节、提供者 16 字节、主密钥版本 4 字节、主密钥 16 字节，第 40 个字节起的 4 个字节是标志。
    let blob = protect(b"secret").expect("加得了密");
    let flags = u32::from_le_bytes(blob[40..44].try_into().expect("有 4 个字节"));
    assert_ne!(flags & CRYPTPROTECT_LOCAL_MACHINE, 0, "标志是 {flags:#x}");
}
