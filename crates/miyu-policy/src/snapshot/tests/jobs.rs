//! 子会话回报的正文怎么截进快照（施工 7-6；施工 3-8 五补从 `tests.rs` 挪出来，那边放不下了）。

use super::*;

/// 子会话回报的正文怎么截（施工 7-6）：出厂的快照带着 30000 和截在中间的那一行；以前造的快照里没有，读成出厂的数、空的
/// 那一行，读进来再写出去一字不差。
#[test]
fn report_numbers_and_the_omitted_line_go_in_and_older_snapshots_lack_them() {
    let snapshot = engineer();
    let reports = snapshot.policy().unwrap().reports;
    assert_eq!(reports.chars, 30_000);
    let count = std::collections::BTreeMap::from([("count", "12")]);
    assert_eq!(
        reports.omitted.render(&count).unwrap(),
        "[... 12 characters omitted ...]\n"
    );
    let text = String::from_utf8(snapshot.to_bytes()).unwrap();
    let tail = format!("{JOB_NUMBERS}{RECAP_NUMBERS}{TITLE_NUMBERS}}}");
    assert!(text.ends_with(&tail), "{text}");
    let omitted = r#","subagent_omitted":"[... {count} characters omitted ...]\n""#;
    assert!(text.contains(omitted), "{text}");
    let older = text.replace(JOB_NUMBERS, "").replace(omitted, "");
    let read = Snapshot::from_bytes(older.as_bytes()).unwrap();
    assert_eq!(read.to_bytes(), older.as_bytes(), "读进来再写出去一字不差");
    let reports = read.policy().unwrap().reports;
    assert_eq!(reports.chars, 30_000);
    assert_eq!(reports.omitted.render(&count).unwrap(), "");
    // 快照里的数照快照的。
    let mut smaller = snapshot.clone();
    smaller.jobs = Some(crate::JobNumbers { report_chars: 7 });
    assert_eq!(smaller.policy().unwrap().reports.chars, 7);
}
