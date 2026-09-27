use super::*;
use crate::test_support::Scratch;

#[test]
fn a_token_is_64_hex_and_read_back_as_written() {
    let scratch = Scratch::new();
    let root = scratch.root();
    let token = renew_token(&root).expect("换得了");
    assert_eq!(token.len(), 64);
    assert!(
        token
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
        "{token}"
    );
    assert_eq!(read_token(&root).expect("读得到"), token);
    assert_eq!(
        fs::read_to_string(root.run().join(TOKEN)).expect("读得到"),
        format!("{token}\n")
    );
}

#[test]
fn every_renewal_is_a_new_token() {
    let scratch = Scratch::new();
    let root = scratch.root();
    let first = renew_token(&root).expect("换得了");
    let second = renew_token(&root).expect("换得了");
    assert_ne!(first, second);
    assert_eq!(read_token(&root).expect("读得到"), second);
}

#[cfg(unix)]
#[test]
fn only_the_owner_can_read_the_token() {
    use std::os::unix::fs::PermissionsExt;
    let scratch = Scratch::new();
    let root = scratch.root();
    // 上次崩在半路留下的临时文件，别人也读得了：换令牌时删掉重建，不沿用它的权限。
    let temporary = root.run().join("token.tmp");
    fs::write(&temporary, "left over").expect("写得进");
    fs::set_permissions(&temporary, fs::Permissions::from_mode(0o644)).expect("改得了");
    renew_token(&root).expect("换得了");
    let mode = fs::metadata(root.run().join(TOKEN))
        .expect("在")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
    assert!(!temporary.exists());
}

#[test]
fn the_location_is_one_line_and_read_back_as_written() {
    let scratch = Scratch::new();
    let root = scratch.root();
    let socket = scratch.path().join("some dir").join("core.sock");
    write_location(&root, &socket).expect("写得进");
    assert_eq!(read_location(&root).expect("读得到"), socket);
    let line = fs::read(root.run().join(LOCATION)).expect("读得到");
    let mut expected = socket.as_os_str().as_encoded_bytes().to_vec();
    expected.push(b'\n');
    assert_eq!(line, expected);
}

#[test]
fn a_relative_location_is_refused() {
    let scratch = Scratch::new();
    let root = scratch.root();
    fs::write(root.run().join(LOCATION), "core.sock\n").expect("写得进");
    let error = read_location(&root).expect_err("不是绝对路径");
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
}

#[cfg(unix)]
#[test]
fn a_location_that_is_not_utf8_is_read_back_as_written() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;
    let scratch = Scratch::new();
    let root = scratch.root();
    let socket = PathBuf::from(OsString::from_vec(b"/tmp/\xff\xfe/core.sock".to_vec()));
    write_location(&root, &socket).expect("写得进");
    assert_eq!(read_location(&root).expect("读得到"), socket);
}
