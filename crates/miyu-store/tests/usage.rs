//! 用量汇总（施工 8-15，`docs/blueprint/models.md`「守着它的」）：一次请求一行，重复写不出两行；落后的、没写上的补多出来的；
//! 表坏了、版本不对、删了的照日志重建；回收处里的会话照样算；撤掉的回合里的照样算；没发出去的不算；金额照币种各加各的；
//! 分组、子会话一起算；一次性调用写进账号日志、照用途分组、重建以后还在。删掉的会话、时区在 `usage_purged.rs`。

mod support;

use std::fs;

use miyu_kernel::id::AccountId;
use miyu_store::journal;
use miyu_store::log::SessionLog;
use miyu_store::sqlite::Opened;
use miyu_store::usage::{FILE, Group, OneShotCall, Query, UsageIndex};

use support::*;

/// 一个主会话：三次请求（一次带美元的金额、一次带人民币的、一次没金额），中间一次没发出去的、一条人说的。
fn spoken(root: &miyu_store::root::DataRoot, index: &UsageIndex) -> SessionLog {
    let id = session(1);
    let mut log = write_log(&root.session_dir(&admin(), &id), &[]);
    let events = [
        created(at(0), None),
        said(2, at(1)),
        called(
            3,
            at(2),
            "deepseek-flash",
            Some([100, 20, 0, 10]),
            &cost("0.25", "USD"),
            "",
        ),
        unsent(4, at(3)),
        called(
            5,
            at(4),
            "deepseek-flash",
            Some([5, 0, 0, 1]),
            &cost("1.5", "CNY"),
            "",
        ),
        called(
            6,
            at(5),
            "deepseek-pro",
            Some([7, 0, 3, 2]),
            "",
            r#","purpose":"title""#,
        ),
    ];
    land(index, &mut log, &id, &who(None), &events);
    log
}

#[test]
fn each_sent_request_is_one_row_and_amounts_add_up_by_currency() {
    let scratch = Scratch::new("usage-rows");
    let root = root_in(&scratch);
    let (index, opened) = UsageIndex::open(&root);
    assert!(matches!(opened, Opened::Created));
    assert_eq!(index.path(), root.state().join(FILE));
    spoken(&root, &index);
    let all = total(&index);
    assert_eq!(all.requests, 3, "没发出去的不算");
    assert_eq!(all.usage, usage(112, 20, 3, 13));
    assert_eq!(all.unpriced, 1);
    assert_eq!(
        all.amounts,
        [("CNY".to_string(), 1.5), ("USD".to_string(), 0.25)],
        "不换算、不相加，照币种代码排"
    );
}

#[test]
fn writing_the_same_batch_again_adds_nothing() {
    let scratch = Scratch::new("usage-again");
    let root = root_in(&scratch);
    let (index, _) = UsageIndex::open(&root);
    let id = session(1);
    let log = spoken(&root, &index);
    let events = miyu_store::log::read_events(log.dir()).unwrap();
    let start = miyu_store::log::Mark {
        segment: 1,
        bytes: 0,
        next: miyu_kernel::id::Seq::FIRST,
    };
    index
        .advance(&id, &who(None), &start, &events, &log.mark())
        .unwrap();
    assert_eq!(total(&index).requests, 3);
    assert_eq!(total(&index).requests, 3, "补过以后再补也一样");
}

#[test]
fn what_was_never_written_is_caught_up_from_the_log() {
    let scratch = Scratch::new("usage-catch-up");
    let root = root_in(&scratch);
    let (index, _) = UsageIndex::open(&root);
    let id = session(1);
    let mut log = spoken(&root, &index);
    // 落了盘、没写进汇总（崩在写之前）：只多出来的那一截补上。
    log.append(&[called(
        7,
        at(6),
        "deepseek-flash",
        Some([1, 0, 0, 1]),
        &cost("0.5", "USD"),
        "",
    )])
    .unwrap();
    // 后来的一批照常写：位置接不上，行照写、位置不挪。
    land(
        &index,
        &mut log,
        &id,
        &who(None),
        &[called(
            8,
            at(7),
            "deepseek-flash",
            Some([2, 0, 0, 2]),
            &cost("0.25", "USD"),
            "",
        )],
    );
    let all = total(&index);
    assert_eq!(all.requests, 5);
    assert_eq!(all.usage, usage(115, 20, 3, 16));
    assert_eq!(all.amounts[1], ("USD".to_string(), 1.0));
    // 一个从没写过的会话（核心以前造的）：整份读进来。
    let old = session(2);
    write_log(
        &root.session_dir(&admin(), &old),
        &[
            created(at(10), None),
            called(2, at(11), "deepseek-flash", Some([1, 1, 1, 1]), "", ""),
        ],
    );
    assert_eq!(total(&index).requests, 6);
}

#[test]
fn a_reverted_turn_still_counts() {
    let scratch = Scratch::new("usage-reverted");
    let root = root_in(&scratch);
    let (index, _) = UsageIndex::open(&root);
    let id = session(1);
    let mut log = spoken(&root, &index);
    land(
        &index,
        &mut log,
        &id,
        &who(None),
        &[line(7, at(6), "turn.reverted", r#"{"turns":[2]}"#)],
    );
    assert_eq!(total(&index).requests, 3, "钱已经花了");
}

#[test]
fn a_broken_or_outdated_table_is_rebuilt_from_the_logs() {
    let scratch = Scratch::new("usage-rebuild");
    let root = root_in(&scratch);
    let path = root.state().join(FILE);
    {
        let (index, _) = UsageIndex::open(&root);
        spoken(&root, &index);
        assert_eq!(total(&index).requests, 3);
    }
    let before = {
        let (index, opened) = UsageIndex::open(&root);
        assert!(matches!(opened, Opened::Kept));
        total(&index)
    };
    // 版本不对：删掉建空的，照日志补满。
    {
        let db = rusqlite::Connection::open(&path).unwrap();
        db.pragma_update(None, "user_version", 99).unwrap();
    }
    let (index, opened) = UsageIndex::open(&root);
    assert!(matches!(opened, Opened::Rebuilt(_)), "{opened:?}");
    assert_eq!(total(&index), before);
    drop(index);
    // 乱写的文件。
    forget(&root);
    fs::write(&path, b"not a database at all, just text").unwrap();
    let (index, opened) = UsageIndex::open(&root);
    assert!(matches!(opened, Opened::Rebuilt(_)), "{opened:?}");
    assert_eq!(total(&index), before);
    // 用着用着坏了：删掉重建。
    index.reset().unwrap();
    assert_eq!(total(&index), before);
}

#[test]
fn sessions_in_the_trash_still_count() {
    let scratch = Scratch::new("usage-trash");
    let root = root_in(&scratch);
    let (index, _) = UsageIndex::open(&root);
    drop(spoken(&root, &index));
    miyu_store::trash::discard(&root, &admin(), &session(1), at(60)).unwrap();
    drop(index);
    forget(&root);
    let (rebuilt, _) = UsageIndex::open(&root);
    assert_eq!(total(&rebuilt).requests, 3, "回收处里的照样读");
}

#[test]
fn groups_split_by_model_purpose_and_session_and_a_tree_takes_the_children() {
    let scratch = Scratch::new("usage-groups");
    let root = root_in(&scratch);
    let (index, _) = UsageIndex::open(&root);
    drop(spoken(&root, &index));
    let (parent, child) = (session(1), session(3));
    let mut log = write_log(&root.session_dir(&admin(), &child), &[]);
    land(
        &index,
        &mut log,
        &child,
        &who(Some(&parent)),
        &[
            created(at(20), Some(&parent)),
            called(2, at(21), "deepseek-pro", Some([1, 0, 0, 1]), "", ""),
        ],
    );
    let by_model = totals(&index, &query(&[Group::Model]));
    let shown: Vec<(Option<String>, u64)> = by_model
        .iter()
        .map(|total| (total.keys[0].clone(), total.requests))
        .collect();
    assert_eq!(
        shown,
        [
            (Some("deepseek/deepseek-flash".to_string()), 2),
            (Some("deepseek/deepseek-pro".to_string()), 2)
        ]
    );
    let by_purpose = totals(&index, &query(&[Group::Purpose, Group::Session]));
    let shown: Vec<(Vec<Option<String>>, u64)> = by_purpose
        .iter()
        .map(|total| (total.keys.clone(), total.requests))
        .collect();
    let (p, c) = (parent.as_str().to_string(), child.as_str().to_string());
    assert_eq!(
        shown,
        [
            (vec![None, Some(p.clone())], 2),
            (vec![None, Some(c)], 1),
            (vec![Some("title".to_string()), Some(p)], 1),
        ],
        "照分组的几格排，空的在前"
    );
    let only = |tree: bool| {
        let query = Query {
            session: Some((parent.clone(), tree)),
            ..query(&[])
        };
        totals(&index, &query)[0].requests
    };
    assert_eq!((only(false), only(true)), (3, 4), "连子会话一起");
    let range = Query {
        from: Some(at(4)),
        until: Some(at(21)),
        ..query(&[])
    };
    assert_eq!(totals(&index, &range)[0].requests, 2, "左闭右开");
}

#[test]
fn one_shot_calls_go_to_the_account_journal_and_survive_a_rebuild() {
    let scratch = Scratch::new("usage-oneshot");
    let root = root_in(&scratch);
    let (index, _) = UsageIndex::open(&root);
    let call = OneShotCall {
        purpose: "vision".to_string(),
        endpoint: "deepseek".to_string(),
        model: "deepseek-flash".to_string(),
        usage: Some(usage(812, 0, 0, 9)),
        cost: None,
    };
    index.one_shot(&admin(), at(30), &call).unwrap();
    index.one_shot(&admin(), at(31), &call).unwrap();
    let journal = fs::read_to_string(root.account_dir(&admin()).join(journal::FILE)).unwrap();
    assert_eq!(journal.lines().count(), 2);
    assert!(journal.contains(r#""kind":"usage.oneshot""#), "{journal}");
    let by_purpose = |index: &UsageIndex| {
        let totals = totals(index, &query(&[Group::Purpose, Group::Session]));
        totals
            .iter()
            .map(|total| (total.keys.clone(), total.requests, total.unpriced))
            .collect::<Vec<_>>()
    };
    let expected = vec![(vec![Some("vision".to_string()), None], 2, 2)];
    assert_eq!(by_purpose(&index), expected, "没有会话，有用途");
    drop(index);
    forget(&root);
    let (rebuilt, _) = UsageIndex::open(&root);
    assert_eq!(by_purpose(&rebuilt), expected, "照账号日志读回来");
    let other = AccountId::parse("bob").unwrap();
    assert!(
        totals(&rebuilt, &query(&[Group::Account]))
            .iter()
            .all(|total| total.keys[0].as_deref() != Some(other.as_str()))
    );
}

#[test]
fn a_later_segment_is_caught_up_too() {
    let scratch = Scratch::new("usage-segments");
    let root = root_in(&scratch);
    let (index, _) = UsageIndex::open(&root);
    let id = session(5);
    let dir = root.session_dir(&admin(), &id);
    let mut log = SessionLog::create(&dir, 1).unwrap();
    log.append(&[created(at(0), None)]).unwrap();
    log.append(&[called(
        2,
        at(1),
        "deepseek-flash",
        Some([1, 0, 0, 1]),
        "",
        "",
    )])
    .unwrap();
    assert_eq!(total(&index).requests, 1);
    log.append(&[called(
        3,
        at(2),
        "deepseek-flash",
        Some([1, 0, 0, 1]),
        "",
        "",
    )])
    .unwrap();
    assert_eq!(total(&index).requests, 2, "换了段的接着读");
}
