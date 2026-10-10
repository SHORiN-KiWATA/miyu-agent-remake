use std::path::PathBuf;

use serde_json::json;

use super::Avatars;

fn dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("miyu-avatars-{tag}-{}", std::process::id()));
    drop(std::fs::remove_dir_all(&dir));
    dir
}

#[test]
fn one_avatar_is_asked_at_a_time_and_kept_by_its_version() {
    let dir = dir("keep");
    let mut book = Avatars::at(dir.clone());
    let wanted = [("miyu", "aaaa"), ("cat", "bbbb")];
    assert_eq!(book.next(&wanted).as_deref(), Some("miyu"));
    assert_eq!(book.next(&wanted), None, "一次只要一张");
    book.answered(Some(
        &json!({"avatar": "aaaa", "media_type": "image/png", "data": "AQID"}),
    ));
    let file = book.file("aaaa").expect("存下了");
    assert_eq!(std::fs::read(&file).unwrap(), [1, 2, 3]);
    assert_eq!(book.next(&wanted).as_deref(), Some("cat"), "存过的不再要");
    let again = Avatars::at(dir.clone());
    assert_eq!(again.file("aaaa"), Some(file), "重启以后照存下的");
    drop(std::fs::remove_dir_all(&dir));
}

#[test]
fn a_refused_or_changed_avatar_is_not_asked_again_this_time() {
    let dir = dir("refused");
    let mut book = Avatars::at(dir.clone());
    assert!(book.next(&[("miyu", "aaaa")]).is_some());
    book.answered(None);
    assert_eq!(book.file("aaaa"), None);
    assert_eq!(book.next(&[("miyu", "aaaa")]), None, "要不到的这次不再要");
    // 中途换过头像：回来的是新版本，照它存；要的那个旧版本记成要不到。
    assert!(book.next(&[("cat", "cccc")]).is_some());
    book.answered(Some(
        &json!({"avatar": "dddd", "media_type": "image/png", "data": "AQID"}),
    ));
    assert!(book.file("dddd").is_some());
    assert_eq!(book.next(&[("cat", "cccc")]), None);
    drop(std::fs::remove_dir_all(&dir));
}

#[test]
fn without_a_cache_directory_nothing_is_asked() {
    let mut book = Avatars::default();
    assert_eq!(book.next(&[("miyu", "aaaa")]), None);
}
