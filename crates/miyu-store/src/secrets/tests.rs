//! 密钥文件（施工 8-5）：没有的是空的；写的新建、替换都是 0600，临时文件建的时候就是 0600，手改松了的写一次收回来；
//! 组、别人读得到的照读、说出来；`Debug` 不印字；顺着链接写。

use std::fs;

use super::*;
use crate::config_file::version;
use crate::test_support::Scratch;

/// 一眼看得出是假的 key。
const FAKE: &str = "sk-FAKE-KEY-FOR-TESTS-0001";

#[test]
fn a_missing_file_is_nothing_and_a_written_one_reads_back() {
    let temp = Scratch::new();
    let path = temp.path().join("system").join(FILE);
    assert!(read(&path).unwrap().is_none());
    let content = format!("deepseek = \"{FAKE}\"\n");
    write(&path, content.as_bytes(), None).unwrap();
    let stored = read(&path).unwrap().unwrap();
    assert_eq!(stored.text.text, content);
    assert_eq!(stored.text.version, version(content.as_bytes()));
    assert!(!stored.open, "自己写的别人读不到");
    let shown = format!("{stored:?}");
    assert!(!shown.contains(FAKE), "{shown}");
}

#[test]
fn a_changed_file_is_not_overwritten() {
    let temp = Scratch::new();
    let path = temp.path().join(FILE);
    write(&path, b"a = \"1\"\n", None).unwrap();
    let stale = version(b"something else");
    assert!(matches!(
        write(&path, b"a = \"2\"\n", Some(&stale)),
        Err(WriteError::Changed)
    ));
    assert_eq!(fs::read(&path).unwrap(), b"a = \"1\"\n");
}

#[cfg(unix)]
#[test]
fn new_and_replaced_files_are_private_even_the_temporary_one() {
    use std::os::unix::fs::PermissionsExt;
    let temp = Scratch::new();
    fs::create_dir_all(temp.path()).unwrap();
    let path = temp.path().join(FILE);
    let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
    let mut seen = None;
    config_file::write_with(&path, b"a = \"1\"\n", None, Mode::Private, &mut |temp| {
        seen = Some(mode(temp));
        Ok(())
    })
    .unwrap();
    assert_eq!(seen, Some(0o600), "临时文件建的时候就是 0600");
    assert_eq!(mode(&path), 0o600);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    let stored = read(&path).unwrap().unwrap();
    assert!(stored.open, "别人读得到的说出来");
    write(&path, b"a = \"2\"\n", Some(&stored.text.version)).unwrap();
    assert_eq!(mode(&path), 0o600, "替换的收回 0600");
}

#[cfg(unix)]
#[test]
fn group_or_others_reading_is_open() {
    use std::os::unix::fs::PermissionsExt;
    let temp = Scratch::new();
    fs::create_dir_all(temp.path()).unwrap();
    let path = temp.path().join(FILE);
    fs::write(&path, "a = \"1\"\n").unwrap();
    for (bits, open) in [
        (0o600, false),
        (0o640, true),
        (0o604, true),
        (0o700, false),
        (0o620, false),
    ] {
        fs::set_permissions(&path, fs::Permissions::from_mode(bits)).unwrap();
        assert_eq!(read(&path).unwrap().unwrap().open, open, "{bits:o}");
    }
}

#[cfg(unix)]
#[test]
fn a_link_is_followed_and_left_alone() {
    use std::os::unix::fs::PermissionsExt;
    let temp = Scratch::new();
    fs::create_dir_all(temp.path()).unwrap();
    let real = temp.path().join("real.toml");
    let link = temp.path().join(FILE);
    std::os::unix::fs::symlink(&real, &link).unwrap();
    write(&link, b"a = \"1\"\n", None).unwrap();
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(fs::read(&real).unwrap(), b"a = \"1\"\n");
    assert_eq!(
        fs::metadata(&real).unwrap().permissions().mode() & 0o777,
        0o600
    );
}
