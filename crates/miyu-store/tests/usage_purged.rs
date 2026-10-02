//! 删掉的会话的用量（施工 8-15，`docs/blueprint/models.md`「怎么走」第九条第 5 条）：回收处清它之前往账号日志写
//! `usage.purged`，写进去了才删目录；汇总照它换掉单次行，加起来一样多，删了汇总重建也一样；写了两遍不加两遍；写不进账号
//! 日志的不删。分天：整点时区的和单次行一分不差，半点时区照小时的开头归到哪天。

mod support;

use std::fs;
use std::time::Duration;

use miyu_kernel::time::UtcOffset;
use miyu_store::journal;
use miyu_store::root::DataRoot;
use miyu_store::trash::{self, DELETED_AT};
use miyu_store::usage::purged::{self, Purged};
use miyu_store::usage::{Group, Query, Total, UsageIndex};

use support::*;

/// 留 7 天。
const WEEK: Duration = Duration::from_secs(7 * 24 * 3600);

/// 一个子会话（父会话是 `session(1)`）在 2026-10-01 跨了两天（UTC）：08:30、23:20、23:50，次日 00:10，主请求带美元、
/// 回顾带人民币，一次没金额。
fn spoken(root: &DataRoot, index: &UsageIndex) {
    let id = session(2);
    let mut log = write_log(&root.session_dir(&admin(), &id), &[]);
    let parent = session(1);
    land(
        index,
        &mut log,
        &id,
        &who(Some(&parent)),
        &[
            created(at(0), Some(&parent)),
            called(
                2,
                at(510),
                "deepseek-flash",
                Some([10, 0, 0, 1]),
                &cost("0.125", "USD"),
                "",
            ),
            called(
                3,
                at(1400),
                "deepseek-flash",
                Some([20, 5, 0, 2]),
                &cost("0.25", "USD"),
                "",
            ),
            called(
                4,
                at(1430),
                "deepseek-flash",
                Some([30, 0, 0, 3]),
                &cost("2", "CNY"),
                r#","purpose":"recap""#,
            ),
            called(5, at(1450), "deepseek-flash", Some([40, 0, 0, 4]), "", ""),
        ],
    );
}

/// 挪进回收处，八天以后清：交回清了几个。
fn purge_later(root: &DataRoot) -> trash::Purged {
    trash::discard(root, &admin(), &session(2), at(2000)).expect("挪得进回收处");
    trash::purge(root, &admin(), at(2000 + 8 * 24 * 60), WEEK).expect("回收处读得了")
}

/// 照 `group` 分组、照 `offset` 分天的几行：分组的几格、请求数、用量、金额、没金额的次数。
fn rows(index: &UsageIndex, group: &[Group], minutes: i32) -> Vec<Total> {
    let query = Query {
        offset: UtcOffset::from_minutes(minutes).expect("时区在范围里"),
        ..query(group)
    };
    totals(index, &query)
}

#[test]
fn purging_keeps_the_usage_and_the_totals_stay_the_same() {
    let scratch = Scratch::new("usage-purge");
    let root = root_in(&scratch);
    let (index, _) = UsageIndex::open(&root);
    spoken(&root, &index);
    let groups = [
        Group::Model,
        Group::Purpose,
        Group::Session,
        Group::Venue,
        Group::Account,
    ];
    let before = (total(&index), rows(&index, &groups, 0));
    let purged = purge_later(&root);
    assert_eq!((purged.removed, purged.failed.clone()), (1, Vec::new()));
    assert!(
        !root
            .trashed_sessions(&admin())
            .join(session(2).as_str())
            .exists()
    );
    let journal = fs::read_to_string(root.account_dir(&admin()).join(journal::FILE)).unwrap();
    assert_eq!(journal.lines().count(), 1);
    assert!(journal.contains(r#""kind":"usage.purged""#), "{journal}");
    let after = (total(&index), rows(&index, &groups, 0));
    assert_eq!(after, before, "换成按小时的合计，加起来一样");
    // 删了汇总重建：照账号日志读回来。
    drop(index);
    forget(&root);
    let (rebuilt, _) = UsageIndex::open(&root);
    assert_eq!((total(&rebuilt), rows(&rebuilt, &groups, 0)), before);
    // 父会话连子会话一起算：删掉的子会话也算在里面。
    let tree = Query {
        session: Some((session(1), true)),
        ..query(&[])
    };
    assert_eq!(totals(&rebuilt, &tree)[0].requests, 4);
}

#[test]
fn the_summary_is_hourly_by_provider_model_and_purpose() {
    let scratch = Scratch::new("usage-summary");
    let root = root_in(&scratch);
    let (index, _) = UsageIndex::open(&root);
    spoken(&root, &index);
    let dir = root.session_dir(&admin(), &session(2));
    let summary = purged::of_log(&dir, &session(2)).unwrap().unwrap();
    let written = serde_json::to_string(&summary).unwrap();
    assert_eq!(
        written,
        concat!(
            r#"{"session":"01900000-0000-7000-8000-000000000002","owner":"admin","venue":"local","parent":"01900000-0000-7000-8000-000000000001","spent":["#,
            r#"{"provider":"deepseek","model":"deepseek-flash","hour":"2026-10-01T08:00:00.000Z","requests":1,"unpriced":0,"usage":{"uncached":10,"cache_read":0,"cache_write":0,"output":1},"amounts":[{"currency":"USD","amount":0.125}]},"#,
            r#"{"provider":"deepseek","model":"deepseek-flash","hour":"2026-10-01T23:00:00.000Z","requests":1,"unpriced":0,"usage":{"uncached":20,"cache_read":5,"cache_write":0,"output":2},"amounts":[{"currency":"USD","amount":0.25}]},"#,
            r#"{"provider":"deepseek","model":"deepseek-flash","purpose":"recap","hour":"2026-10-01T23:00:00.000Z","requests":1,"unpriced":0,"usage":{"uncached":30,"cache_read":0,"cache_write":0,"output":3},"amounts":[{"currency":"CNY","amount":2}]},"#,
            r#"{"provider":"deepseek","model":"deepseek-flash","hour":"2026-10-02T00:00:00.000Z","requests":1,"unpriced":1,"usage":{"uncached":40,"cache_read":0,"cache_write":0,"output":4},"amounts":[]}]}"#
        )
    );
    let back: Purged = serde_json::from_str(&written).unwrap();
    assert_eq!(back, summary);
    // 一次请求都没有的不写。
    let empty = session(9);
    write_log(&root.session_dir(&admin(), &empty), &[created(at(0), None)]);
    assert_eq!(
        purged::of_log(&root.session_dir(&admin(), &empty), &empty).unwrap(),
        None
    );
}

#[test]
fn whole_hour_offsets_split_days_exactly_and_half_hour_ones_by_the_hour_start() {
    let scratch = Scratch::new("usage-days");
    let root = root_in(&scratch);
    let (index, _) = UsageIndex::open(&root);
    spoken(&root, &index);
    let days = |index: &UsageIndex, minutes: i32| -> Vec<(Option<String>, u64)> {
        rows(index, &[Group::Day], minutes)
            .iter()
            .map(|total| (total.keys[0].clone(), total.requests))
            .collect()
    };
    let utc = days(&index, 0);
    let tokyo = days(&index, 540);
    let india = days(&index, 330);
    let date = |text: &str| Some(text.to_string());
    assert_eq!(utc, [(date("2026-10-01"), 3), (date("2026-10-02"), 1)]);
    assert_eq!(tokyo, [(date("2026-10-01"), 1), (date("2026-10-02"), 3)]);
    // 单次的：23:20 UTC 是印度 04:50，次日；08:30 UTC 是 14:00，当天。
    assert_eq!(india, [(date("2026-10-01"), 1), (date("2026-10-02"), 3)]);
    purge_later(&root);
    assert_eq!(days(&index, 0), utc, "整点的一分不差");
    assert_eq!(days(&index, 540), tokyo, "整点的一分不差");
    // 按小时存：08:00 那一小时在印度是 13:30，当天；照样对得上。
    assert_eq!(days(&index, 330), india);
}

#[test]
fn a_half_hour_offset_puts_a_purged_hour_on_the_day_it_starts() {
    let scratch = Scratch::new("usage-half");
    let root = root_in(&scratch);
    let (index, _) = UsageIndex::open(&root);
    let id = session(2);
    let mut log = write_log(&root.session_dir(&admin(), &id), &[]);
    // 18:40 UTC 是印度的次日 00:10；那一小时 18:00 起，是印度的当天 23:30。
    land(
        &index,
        &mut log,
        &id,
        &who(None),
        &[
            created(at(0), None),
            called(
                2,
                at(18 * 60 + 40),
                "deepseek-flash",
                Some([1, 0, 0, 1]),
                "",
                "",
            ),
        ],
    );
    drop(log);
    let day = |index: &UsageIndex| rows(index, &[Group::Day], 330)[0].keys[0].clone();
    assert_eq!(day(&index).as_deref(), Some("2026-10-02"));
    purge_later(&root);
    assert_eq!(
        day(&index).as_deref(),
        Some("2026-10-01"),
        "照那一小时的开头"
    );
}

#[test]
fn a_summary_written_twice_counts_once() {
    let scratch = Scratch::new("usage-twice");
    let root = root_in(&scratch);
    let (index, _) = UsageIndex::open(&root);
    spoken(&root, &index);
    let before = total(&index);
    // 崩在写完账号日志、删目录之前：目录还在，下次再清又写一条。
    let dir = root.session_dir(&admin(), &session(2));
    let summary = purged::of_log(&dir, &session(2)).unwrap().unwrap();
    let entry = || journal::Entry {
        at: at(3000),
        by: miyu_kernel::origin::By::Kernel,
        cause: None,
        kind: miyu_kernel::id::EventKind::parse("usage.purged").unwrap(),
        body: serde_json::from_str(&serde_json::to_string(&summary).unwrap()).unwrap(),
    };
    let path = root.account_dir(&admin()).join(journal::FILE);
    journal::append(&path, entry()).unwrap();
    assert_eq!(total(&index), before, "目录还在也只算一处");
    journal::append(&path, entry()).unwrap();
    purge_later(&root);
    assert_eq!(total(&index), before);
    drop(index);
    forget(&root);
    let (rebuilt, _) = UsageIndex::open(&root);
    assert_eq!(total(&rebuilt), before, "重建也只算一次");
}

#[test]
fn without_a_journal_the_session_is_kept() {
    let scratch = Scratch::new("usage-kept");
    let root = root_in(&scratch);
    let (index, _) = UsageIndex::open(&root);
    spoken(&root, &index);
    // 账号日志的位置是个目录：写不进去。
    fs::create_dir_all(root.account_dir(&admin()).join(journal::FILE)).unwrap();
    let purged = purge_later(&root);
    assert_eq!(purged.removed, 0);
    assert_eq!(purged.failed.len(), 1, "{:?}", purged.failed);
    let kept = root.trashed_sessions(&admin()).join(session(2).as_str());
    assert!(kept.join(DELETED_AT).exists(), "写不进账号日志的不删");
}
