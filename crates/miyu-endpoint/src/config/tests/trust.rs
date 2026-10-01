//! 信任的记录：没有记录的、内容变了的、仓库挪了的是还没问过；信任着、版本一样的算；不信任的不算；一个仓库几条的后面的
//! 盖掉前面的；写法不对的那一条不算。记一个回答（施工 8-3）：新建的带开头的注释；同一个仓库原地换、别的字节不动；没有的
//! 加在末尾、前面空一行；换行照原文件；读不懂的不改。

use std::fs;
use std::path::PathBuf;

use miyu_config::merge::Trust;

use crate::config::trust::{Answer, Record, changed, read, recorded, records, trust_of};

fn record(path: &str, version: &str, trusted: bool) -> Record {
    Record {
        path: path.to_string(),
        version: version.to_string(),
        trusted,
    }
}

fn home() -> PathBuf {
    std::env::temp_dir().join("miyu-trust-home")
}

#[test]
fn a_record_counts_only_for_the_same_repository_and_version() {
    let home = home();
    let repo = home.join("src").join("app");
    let records = [record("~/src/app", "sha256:a", true)];
    assert_eq!(
        trust_of(&records, &repo, "sha256:a", Some(&home)),
        Trust::Trusted
    );
    assert_eq!(
        trust_of(&records, &repo, "sha256:b", Some(&home)),
        Trust::Unknown,
        "内容变了"
    );
    let moved = home.join("src").join("moved");
    assert_eq!(
        trust_of(&records, &moved, "sha256:a", Some(&home)),
        Trust::Unknown,
        "挪了地方"
    );
    assert_eq!(
        trust_of(&[], &repo, "sha256:a", Some(&home)),
        Trust::Unknown,
        "没有记录"
    );
    assert_eq!(
        trust_of(&records, &repo, "sha256:a", None),
        Trust::Unknown,
        "家目录不知道，~ 换不开"
    );
}

#[test]
fn a_distrusted_one_stays_distrusted_and_the_last_record_wins() {
    let home = home();
    let repo = home.join("src").join("app");
    let records = [
        record("~/src/app", "sha256:a", true),
        record("~/src/app", "sha256:a", false),
    ];
    assert_eq!(
        trust_of(&records, &repo, "sha256:a", Some(&home)),
        Trust::Distrusted
    );
    let absolute = [record(&repo.to_string_lossy(), "sha256:a", true)];
    assert_eq!(
        trust_of(&absolute, &repo, "sha256:a", Some(&home)),
        Trust::Trusted,
        "绝对路径也认"
    );
}

#[test]
fn the_file_is_read_and_bad_records_are_skipped() {
    let dir = std::env::temp_dir().join(format!("miyu-trust-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("trust.toml");
    assert_eq!(read(&path), Ok(Vec::new()), "没有的是空的");
    fs::write(
        &path,
        "# 注释\n[[project]]\npath = \"~/a\"\nversion = \"sha256:1\"\ntrusted = true\n\n\
         [[project]]\npath = \"~/b\"\ntrusted = \"yes\"\n",
    )
    .unwrap();
    assert_eq!(read(&path), Ok(vec![record("~/a", "sha256:1", true)]));
    fs::write(&path, "[[project]\n").unwrap();
    assert!(read(&path).is_err(), "写法不对的整份读不进来");
    fs::remove_dir_all(&dir).unwrap();
}

fn answer<'a>(repo: &'a std::path::Path, version: &'a str, trusted: bool) -> Answer<'a> {
    Answer {
        repo,
        shown: "~/src/app",
        version,
        trusted,
    }
}

#[test]
fn a_new_record_file_starts_with_the_header() {
    let home = home();
    let repo = home.join("src").join("app");
    assert_eq!(
        recorded(
            None,
            "头一行。",
            &answer(&repo, "sha256:a", true),
            Some(&home)
        )
        .unwrap(),
        "# 头一行。\n[[project]]\npath = \"~/src/app\"\nversion = \"sha256:a\"\ntrusted = true\n"
    );
}

#[test]
fn answering_again_changes_that_record_in_place() {
    let home = home();
    let repo = home.join("src").join("app");
    let text = "# 我的\n[[project]]\npath = \"~/other\"\nversion = \"sha256:o\"\ntrusted = true\n\n\
                [[project]]\npath = \"~/src/app\" # 这个\ntrusted = true\nversion = \"sha256:a\"\n";
    let again = recorded(
        Some(text),
        "头",
        &answer(&repo, "sha256:b", false),
        Some(&home),
    )
    .unwrap();
    assert_eq!(
        again,
        text.replace(
            "trusted = true\nversion = \"sha256:a\"",
            "trusted = false\nversion = \"sha256:b\""
        ),
        "一个仓库一条，新的盖掉旧的，别的字节不动"
    );
    assert_eq!(records(&again).unwrap().len(), 2);
}

#[test]
fn a_new_repository_is_added_at_the_end() {
    let home = home();
    let repo = home.join("src").join("app");
    let text = "[[project]]\r\npath = \"~/other\"\r\nversion = \"sha256:o\"\r\ntrusted = true";
    assert_eq!(
        recorded(
            Some(text),
            "头",
            &answer(&repo, "sha256:a", true),
            Some(&home)
        )
        .unwrap(),
        format!(
            "{text}\r\n\r\n[[project]]\r\npath = \"~/src/app\"\r\nversion = \"sha256:a\"\r\ntrusted = true\r\n"
        )
    );
    assert!(
        recorded(
            Some("[[project]\n"),
            "头",
            &answer(&repo, "sha256:a", true),
            Some(&home)
        )
        .is_err(),
        "读不懂的不改"
    );
    assert!(
        recorded(
            Some("project = 1\n"),
            "头",
            &answer(&repo, "sha256:a", true),
            Some(&home)
        )
        .is_err(),
        "记完读不懂的不交出去"
    );
}

#[test]
fn a_record_written_without_the_verbatim_prefix_matches_the_real_location() {
    let records = [record(r"C:\src\app", "sha256:a", true)];
    assert_eq!(
        trust_of(
            &records,
            std::path::Path::new(r"\\?\C:\src\app"),
            "sha256:a",
            None
        ),
        Trust::Trusted,
        "Windows 上记下的去掉了 \\\\?\\，真实的位置带着"
    );
    let with = [record(r"\\?\C:\src\app", "sha256:a", true)];
    assert_eq!(
        trust_of(&with, std::path::Path::new(r"C:\src\app"), "sha256:a", None),
        Trust::Trusted
    );
}

/// 手改了记录（施工 8-4）：只报回答变了的、新加的仓库，照最后一条算；删掉的、没变的、前面被盖掉的不报。
#[test]
fn a_hand_edit_reports_only_the_answers_that_changed() {
    let old = [
        record("~/src/app", "sha256:a", true),
        record("~/src/lib", "sha256:b", false),
        record("~/src/gone", "sha256:c", true),
    ];
    let new = [
        record("~/src/lib", "sha256:b", true),
        record("~/src/app", "sha256:x", false),
        record("~/src/app", "sha256:a", true),
        record("~/src/new", "sha256:d", false),
    ];
    assert_eq!(
        changed(&old, &new),
        [
            record("~/src/lib", "sha256:b", true),
            record("~/src/new", "sha256:d", false),
        ]
    );
    assert!(changed(&old, &old).is_empty());
}
