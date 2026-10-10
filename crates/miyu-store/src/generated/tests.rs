//! 生成的文件：没有的写上、目录建上；一样的不写（修改时间不变）；不一样的换掉，旁边不留临时文件；写不成的报错。

use std::fs;
use std::time::{Duration, SystemTime};

use super::*;
use crate::test_support::Scratch;

/// 一个老的修改时间：一样的不写，它就不变。
fn old() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(86_400)
}

fn modified(path: &Path) -> SystemTime {
    fs::metadata(path).unwrap().modified().unwrap()
}

/// 目录里除了 `name` 还有什么。
fn others(dir: &Path, name: &str) -> Vec<String> {
    fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|entry| entry != name)
        .collect()
}

#[test]
fn a_missing_file_is_written_with_its_directory() {
    let temp = Scratch::new();
    let path = temp.path().join("state").join("config").join("a.json");
    assert!(write(&path, b"{}\n").unwrap());
    assert_eq!(fs::read(&path).unwrap(), b"{}\n");
    assert_eq!(
        others(path.parent().unwrap(), "a.json"),
        Vec::<String>::new(),
        "不留临时文件"
    );
}

#[test]
fn the_same_bytes_are_not_written_again() {
    let temp = Scratch::new();
    let path = temp.path().join("a.json");
    assert!(write(&path, b"same").unwrap());
    File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(old())
        .unwrap();
    assert!(!write(&path, b"same").unwrap(), "一样的交回 false");
    assert_eq!(modified(&path), old(), "一样的没碰它");
}

#[test]
fn different_bytes_replace_the_file() {
    let temp = Scratch::new();
    fs::create_dir_all(temp.path()).unwrap();
    let path = temp.path().join("a.json");
    fs::write(&path, b"old and longer").unwrap();
    assert!(write(&path, b"new").unwrap());
    assert_eq!(fs::read(&path).unwrap(), b"new");
    assert_eq!(
        others(temp.path(), "a.json"),
        Vec::<String>::new(),
        "不留临时文件"
    );
}

#[test]
fn a_file_where_the_directory_should_be_is_an_error() {
    let temp = Scratch::new();
    fs::create_dir_all(temp.path()).unwrap();
    fs::write(temp.path().join("config"), b"not a directory").unwrap();
    let path = temp.path().join("config").join("a.json");
    assert!(write(&path, b"x").is_err());
    assert_eq!(
        fs::read(temp.path().join("config")).unwrap(),
        b"not a directory"
    );
}

#[test]
fn temp_names_hide_and_never_repeat() {
    let one = temp_name("reference.toml");
    let two = temp_name("reference.toml");
    assert!(one.starts_with(".reference.toml."), "{one}");
    assert!(one.ends_with(".tmp"), "{one}");
    assert_ne!(one, two);
}
