//! 人格的回收处（施工 P-3 下）：挪进去的整个目录一个字节不变、多一个 `deleted_at`，原处没了，同一个编号删两次各是各的；
//! 清的时候满了时限的删、没满的留、读不出删的时刻的留并报出来、没有回收处什么都不做。

use std::time::Duration;

use super::*;
use crate::env::{Env, Platform};
use crate::test_support::Scratch;

const WEEK: Duration = Duration::from_secs(7 * 24 * 3600);

fn alice() -> AccountId {
    AccountId::parse("alice").unwrap()
}

fn root_in(scratch: &Scratch) -> DataRoot {
    let root = DataRoot::locate(&Env {
        platform: Platform::current(),
        miyu_home: Some(scratch.path().join("data").into_os_string()),
        home: None,
        xdg_cache_home: None,
        local_app_data: None,
        miyu_resources: None,
        exe: None,
    })
    .unwrap();
    root.prepare().unwrap();
    root
}

/// 2026-09-30 12:00 加 `millis` 毫秒。
fn at(millis: i64) -> Timestamp {
    Timestamp::from_unix_millis(1_790_000_000_000 + millis).unwrap()
}

/// 家目录里建一个人格目录，写一份人设，交回目录。
fn persona(root: &DataRoot, id: &str) -> std::path::PathBuf {
    let dir = root.account_dir(&alice()).join("personas").join(id);
    fs::create_dir_all(dir.join("prompts")).unwrap();
    fs::write(dir.join("prompts/persona.md"), "be kind\n").unwrap();
    dir
}

#[test]
fn a_persona_moves_whole_and_twice_is_twice() {
    let scratch = Scratch::new();
    let root = root_in(&scratch);
    let first = persona(&root, "mine");
    discard(&root, &alice(), &first, "mine", at(0)).unwrap();
    assert!(!first.exists(), "原处没了");
    let trashed = root
        .trashed_personas(&alice())
        .join(format!("mine.{}", at(0).unix_millis()));
    assert_eq!(
        fs::read_to_string(trashed.join("prompts/persona.md")).unwrap(),
        "be kind\n"
    );
    assert_eq!(
        fs::read_to_string(trashed.join(DELETED_AT)).unwrap(),
        format!("{}\n", at(0))
    );
    let second = persona(&root, "mine");
    discard(&root, &alice(), &second, "mine", at(5)).unwrap();
    assert_eq!(
        fs::read_dir(root.trashed_personas(&alice()))
            .unwrap()
            .count(),
        2
    );
    let gone = root.account_dir(&alice()).join("personas").join("nobody");
    assert!(
        discard(&root, &alice(), &gone, "nobody", at(9)).is_err(),
        "不在的报错"
    );
}

#[test]
fn purging_removes_only_the_expired() {
    let scratch = Scratch::new();
    let root = root_in(&scratch);
    assert_eq!(
        purge(&root, &alice(), at(0), WEEK).unwrap().removed,
        0,
        "没有回收处"
    );
    let old = persona(&root, "old");
    discard(&root, &alice(), &old, "old", at(0)).unwrap();
    let fresh = persona(&root, "fresh");
    discard(&root, &alice(), &fresh, "fresh", at(1000)).unwrap();
    let broken = root.trashed_personas(&alice()).join("broken.1");
    fs::create_dir_all(&broken).unwrap();
    let week = i64::try_from(WEEK.as_millis()).unwrap();
    let purged = purge(&root, &alice(), at(week), WEEK).unwrap();
    assert_eq!(purged.removed, 1, "正好满的删");
    let left: Vec<String> = fs::read_dir(root.trashed_personas(&alice()))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    assert_eq!(
        left,
        [
            "broken.1".to_string(),
            format!("fresh.{}", at(1000).unix_millis())
        ]
    );
    assert_eq!(purged.failed.len(), 1, "读不出删的时刻的留着、报出来");
    assert_eq!(purged.failed[0].0, "broken.1");
}
