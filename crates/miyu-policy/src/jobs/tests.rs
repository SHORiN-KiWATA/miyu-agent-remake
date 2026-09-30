//! 人停的那一句进快照（施工 7-2 补，`docs/blueprint/policy.md` 的 `CoreTexts` 那张表里的 `jobs`）：出厂的快照带着、交给
//! 组装器；以前造的快照里没有，读成空的，读进来再写出去一字不差。

use crate::snapshot::Snapshot;
use crate::test_support::*;

#[test]
fn the_stopped_by_user_line_goes_in_and_older_snapshots_lack_it() {
    let snapshot = engineer();
    let line = include_str!("../../../../resources/core/jobs/stopped-by-user.txt");
    let jobs = snapshot.core.jobs.as_ref().expect("出厂的有回报的写法");
    assert_eq!(jobs.rendered().unwrap().stopped_by_user, line);
    let text = String::from_utf8(snapshot.to_bytes()).unwrap();
    let field = r#","stopped_by_user":"The user stopped this.\n""#;
    assert!(text.contains(field), "{text}");
    let older = text.replace(field, "");
    let read = Snapshot::from_bytes(older.as_bytes()).unwrap();
    assert_eq!(read.to_bytes(), older.as_bytes(), "读进来再写出去一字不差");
    let jobs = read.core.jobs.expect("别的写法还在");
    assert_eq!(jobs.rendered().unwrap().stopped_by_user, "");
}
