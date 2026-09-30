//! 信任的记录：没有记录的、内容变了的、仓库挪了的是还没问过；信任着、版本一样的算；不信任的不算；一个仓库几条的后面的
//! 盖掉前面的；写法不对的那一条不算。

use std::fs;
use std::path::PathBuf;

use miyu_config::merge::Trust;

use crate::config::trust::{Record, read, trust_of};

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
