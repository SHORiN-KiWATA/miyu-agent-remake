//! 读配置文件：没有的是空的；BOM 去掉、版本照整份字节；太大的不读；不是 UTF-8 的、读不了的报错；顺着链接读。

use std::fs;

use super::*;
use crate::test_support::Scratch;

#[test]
fn a_missing_file_is_an_empty_layer() {
    let temp = Scratch::new();
    assert!(matches!(read(&temp.path().join("config.toml")), Ok(None)));
}

#[test]
fn the_bom_is_dropped_but_counted_in_the_version() {
    let temp = Scratch::new();
    fs::create_dir_all(temp.path()).unwrap();
    let path = temp.path().join("config.toml");
    fs::write(&path, "\u{FEFF}a = 1\r\n").unwrap();
    let read = read(&path).unwrap().unwrap();
    assert_eq!(read.text, "a = 1\r\n", "\\r\\n 照原样");
    assert_eq!(read.version, version("\u{FEFF}a = 1\r\n".as_bytes()));
    assert_ne!(read.version, version(b"a = 1\r\n"), "带 BOM 的字节算版本");
    assert!(read.version.starts_with("sha256:") && read.version.len() == 7 + 64);
    assert_eq!(
        version(b""),
        "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn a_file_over_one_mebibyte_is_not_read() {
    let temp = Scratch::new();
    fs::create_dir_all(temp.path()).unwrap();
    let path = temp.path().join("config.toml");
    fs::write(&path, vec![b'#'; usize::try_from(LIMIT).unwrap()]).unwrap();
    assert!(read(&path).unwrap().is_some(), "正好 1 MiB 的读");
    fs::write(&path, vec![b'#'; usize::try_from(LIMIT).unwrap() + 1]).unwrap();
    assert!(matches!(read(&path), Err(ReadError::TooBig)));
}

#[test]
fn bytes_that_are_not_utf8_are_refused() {
    let temp = Scratch::new();
    fs::create_dir_all(temp.path()).unwrap();
    let path = temp.path().join("config.toml");
    fs::write(&path, b"a = \"\xff\"\n").unwrap();
    assert!(matches!(read(&path), Err(ReadError::NotUtf8)));
}

#[test]
fn a_directory_is_unreadable() {
    let temp = Scratch::new();
    let path = temp.path().join("config.toml");
    fs::create_dir_all(&path).unwrap();
    assert!(matches!(read(&path), Err(ReadError::Unreadable(_))));
}

#[cfg(unix)]
#[test]
fn a_link_is_followed_even_outside() {
    let temp = Scratch::new();
    let elsewhere = temp.path().join("elsewhere");
    fs::create_dir_all(&elsewhere).unwrap();
    fs::write(elsewhere.join("real.toml"), "a = 1\n").unwrap();
    let path = temp.path().join("config.toml");
    std::os::unix::fs::symlink(elsewhere.join("real.toml"), &path).unwrap();
    assert_eq!(read(&path).unwrap().unwrap().text, "a = 1\n");
}
