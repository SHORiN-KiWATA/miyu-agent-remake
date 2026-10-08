//! 检索库（施工 R-1，`docs/blueprint/recall.md`「怎么走」第二条）：建、重开、坏了和版本不对的删掉重建；放进、换掉、拿掉；
//! 两个字的词、一个字都搜得到；中了越多的越靠前；中英混着的；只给几条。

use std::fs;
use std::path::PathBuf;

use miyu_kernel::id::Seq;
use miyu_store::recall::{Edit, Hit, Opened, RecallIndex, Room};

use crate::support::*;

/// 临时数据根里管理员的一个检索库的位置。
fn place(scratch: &Scratch) -> PathBuf {
    root_in(scratch)
        .index(&admin())
        .join("recall")
        .join("test.db")
}

fn keys(hits: &[Hit]) -> Vec<&str> {
    hits.iter().map(|hit| hit.key.as_str()).collect()
}

#[test]
fn a_new_index_is_created_and_kept_on_reopen() {
    let scratch = Scratch::new("recall-reopen");
    let path = place(&scratch);
    let (index, opened) = RecallIndex::open(&path);
    assert!(matches!(opened, Opened::Created), "{opened:?}");
    index.put("a", "她记得你用 N 卡", at(1)).unwrap();
    drop(index);
    let (index, opened) = RecallIndex::open(&path);
    assert!(matches!(opened, Opened::Kept), "{opened:?}");
    assert_eq!(keys(&index.search("N卡", 10).unwrap()), ["a"]);
    assert_eq!(index.keys().unwrap(), ["a"]);
}

#[test]
fn a_broken_or_outdated_file_is_rebuilt_empty() {
    let scratch = Scratch::new("recall-broken");
    let path = place(&scratch);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        &path,
        b"not a database at all, just some bytes that are long enough",
    )
    .unwrap();
    let (index, opened) = RecallIndex::open(&path);
    assert!(matches!(opened, Opened::Rebuilt(_)), "{opened:?}");
    index.put("a", "记忆", at(1)).unwrap();
    assert_eq!(keys(&index.search("记忆", 10).unwrap()), ["a"]);
    drop(index);

    // R-1 造的版本 1 的库（没有 marks）也删掉重建。
    let db = rusqlite::Connection::open(&path).unwrap();
    db.pragma_update(None, "user_version", 1).unwrap();
    drop(db);
    let (index, opened) = RecallIndex::open(&path);
    assert!(matches!(opened, Opened::Rebuilt(_)), "{opened:?}");
    assert!(index.keys().unwrap().is_empty());
}

#[test]
fn replacing_and_removing_change_what_is_found() {
    let scratch = Scratch::new("recall-put");
    let (index, _) = RecallIndex::open(&place(&scratch));
    index.put("a", "今天去看了樱花", at(1)).unwrap();
    index.put("b", "明天要考试", at(2)).unwrap();
    assert_eq!(keys(&index.search("樱花", 10).unwrap()), ["a"]);

    index.put("a", "今天去爬山了", at(3)).unwrap();
    assert!(index.search("樱花", 10).unwrap().is_empty());
    assert_eq!(keys(&index.search("爬山", 10).unwrap()), ["a"]);

    index.remove("a").unwrap();
    index.remove("missing").unwrap();
    assert!(index.search("爬山", 10).unwrap().is_empty());
    assert_eq!(index.keys().unwrap(), ["b"]);
}

#[test]
fn two_character_words_and_single_characters_are_found() {
    let scratch = Scratch::new("recall-short");
    let (index, _) = RecallIndex::open(&place(&scratch));
    index.put("cat", "我的猫很可爱", at(1)).unwrap();
    index.put("memory", "她的记忆系统", at(2)).unwrap();
    assert_eq!(keys(&index.search("猫", 10).unwrap()), ["cat"]);
    assert_eq!(keys(&index.search("记忆", 10).unwrap()), ["memory"]);
    assert_eq!(keys(&index.search("東京", 10).unwrap()), Vec::<&str>::new());
}

#[test]
fn more_matched_terms_rank_first_and_ranks_count_from_zero() {
    let scratch = Scratch::new("recall-rank");
    let (index, _) = RecallIndex::open(&place(&scratch));
    index.put("one", "周末打算去爬山", at(1)).unwrap();
    index
        .put("both", "周末打算去爬山，顺便看樱花", at(2))
        .unwrap();
    index.put("none", "工作日要上班", at(3)).unwrap();
    let hits = index.search("周末去爬山看樱花", 10).unwrap();
    assert_eq!(keys(&hits), ["both", "one"]);
    assert_eq!(hits.iter().map(|hit| hit.rank).collect::<Vec<_>>(), [0, 1]);
    assert_eq!(
        (hits[0].text.as_str(), hits[0].at),
        ("周末打算去爬山，顺便看樱花", at(2)),
        "带回放进来时的字和时刻"
    );
}

#[test]
fn chinese_and_english_mixed() {
    let scratch = Scratch::new("recall-mixed");
    let (index, _) = RecallIndex::open(&place(&scratch));
    index.put("rust", "他在用Rust写一个编辑器", at(1)).unwrap();
    index.put("go", "她用 Go 写后端", at(2)).unwrap();
    assert_eq!(keys(&index.search("rust 编辑器", 10).unwrap()), ["rust"]);
    assert_eq!(keys(&index.search("GO", 10).unwrap()), ["go"]);
}

#[test]
fn only_as_many_as_asked_and_nothing_for_an_empty_query() {
    let scratch = Scratch::new("recall-limit");
    let (index, _) = RecallIndex::open(&place(&scratch));
    for n in 0..5 {
        index.put(&format!("k{n}"), "一样的话", at(n)).unwrap();
    }
    assert_eq!(index.search("一样", 3).unwrap().len(), 3);
    assert!(index.search("，。！", 3).unwrap().is_empty());
    assert!(index.search("一样", 0).unwrap().is_empty());
}

fn seq(n: u64) -> Seq {
    Seq::new(n).expect("序号从 1 起")
}

#[test]
fn a_batch_and_where_its_source_got_to_are_written_together() {
    let scratch = Scratch::new("recall-apply");
    let (index, _) = RecallIndex::open(&place(&scratch));
    assert_eq!(index.mark("s1").unwrap(), None);
    index
        .apply(
            "s1",
            &[
                Edit::Put {
                    key: "s1/3".into(),
                    text: "今天看了樱花".into(),
                    at: at(1),
                },
                Edit::Put {
                    key: "s1/8".into(),
                    text: "明天去爬山".into(),
                    at: at(2),
                },
                Edit::Remove { key: "s1/3".into() },
            ],
            seq(12),
        )
        .unwrap();
    assert_eq!(index.mark("s1").unwrap(), Some(seq(12)));
    assert_eq!(index.keys().unwrap(), ["s1/8"]);
    index.apply("s1", &[], seq(15)).unwrap();
    assert_eq!(index.mark("s1").unwrap(), Some(seq(15)));
}

#[test]
fn forgetting_a_source_takes_only_its_own() {
    let scratch = Scratch::new("recall-forget");
    let (index, _) = RecallIndex::open(&place(&scratch));
    for source in ["s1", "s10", "s2"] {
        let edit = Edit::Put {
            key: format!("{source}/3"),
            text: "一样的话".into(),
            at: at(1),
        };
        index.apply(source, &[edit], seq(5)).unwrap();
    }
    index.forget("s1").unwrap();
    assert_eq!(index.keys().unwrap(), ["s10/3", "s2/3"]);
    assert_eq!(index.mark("s1").unwrap(), None);
    assert_eq!(index.mark("s10").unwrap(), Some(seq(5)));
    assert_eq!(keys(&index.search("一样", 10).unwrap()).len(), 2);
}

/// 量尺（施工单 R-1「验收」第 3 条）：一万条中文短句放进库、找一次各要多久，库多大。只断言结果，不断言耗时。
/// `cargo test -p miyu-store --release --test recall -- --ignored --nocapture`。
#[test]
#[ignore = "量尺"]
fn measure_ten_thousand_sentences() {
    let scratch = Scratch::new("recall-measure");
    let path = place(&scratch);
    let (index, _) = RecallIndex::open(&path);
    let words = [
        "今天",
        "明天",
        "周末",
        "我们",
        "一起",
        "去看",
        "樱花",
        "爬山",
        "写代码",
        "编辑器",
        "考试",
        "显卡",
        "猫咪",
        "吃饭",
        "火锅",
        "电影",
        "游戏",
        "加班",
        "睡觉",
        "下雨",
    ];
    let started = std::time::Instant::now();
    for n in 0..10_000usize {
        let sentence: String = (0..8)
            .map(|k| words[(n * 7 + k * 13 + n / 3) % words.len()])
            .collect();
        index.put(&format!("k{n}"), &sentence, at(0)).unwrap();
    }
    let put = started.elapsed();
    let started = std::time::Instant::now();
    let hits = index.search("周末一起去看樱花吃火锅", 20).unwrap();
    let search = started.elapsed();
    assert_eq!(hits.len(), 20);
    drop(index);
    let size: u64 = ["", "-wal"]
        .iter()
        .map(|suffix| {
            let mut name = path.clone().into_os_string();
            name.push(suffix);
            fs::metadata(name).map(|m| m.len()).unwrap_or(0)
        })
        .sum();
    println!(
        "put 10000: {put:?}, search: {search:?}, size: {} KiB",
        size / 1024
    );
}

#[test]
fn turn_indexes_are_one_per_account_and_persona_and_forgetting_a_session_reaches_all() {
    use miyu_kernel::id::SessionId;
    use miyu_store::recall::RecallIndexes;
    let scratch = Scratch::new("recall-indexes");
    let root = root_in(&scratch);
    let indexes = RecallIndexes::new(&root);
    let (engineer, opened) = indexes.turns(&Room::persona(&admin(), "engineer"));
    assert!(matches!(opened, Some(Opened::Created)), "{opened:?}");
    let (again, opened) = indexes.turns(&Room::persona(&admin(), "engineer"));
    assert!(opened.is_none(), "开过的不再开：{opened:?}");
    assert!(std::sync::Arc::ptr_eq(&engineer, &again));
    assert!(
        root.index(&admin())
            .join("recall")
            .join("turns-engineer.db")
            .is_file()
    );

    let session = SessionId::parse("0192f3a0-1111-7abc-8def-001122334455").unwrap();
    let other = SessionId::parse("0192f3a0-2222-7abc-8def-001122334455").unwrap();
    let (miyu, _) = indexes.turns(&Room::persona(&admin(), "miyu"));
    for index in [&engineer, &miyu] {
        for id in [&session, &other] {
            let edit = Edit::Put {
                key: format!("{id}/3"),
                text: "聊过的话".into(),
                at: at(1),
            };
            index.apply(&id.to_string(), &[edit], seq(4)).unwrap();
        }
    }
    drop(indexes);
    // 新开的登记（核心重启以后）照样找得到磁盘上的每一个回合库。
    let indexes = RecallIndexes::new(&root);
    indexes.forget_session(&admin(), &session).unwrap();
    for persona in ["engineer", "miyu"] {
        let (index, _) = indexes.turns(&Room::persona(&admin(), persona));
        assert_eq!(index.keys().unwrap(), [format!("{other}/3")], "{persona}");
    }
}

/// 量尺（施工单 R-2 上「验收」第 3 条）：一千轮的会话，载入时照整份事件铺回 `TurnFeed`、整份补进回合库要多久。
/// `cargo test -p miyu-store --release --test recall -- --ignored --nocapture`。
#[test]
#[ignore = "量尺"]
fn measure_priming_a_thousand_turns() {
    use miyu_kernel::event::Event;
    use miyu_recall::{Change, TurnFeed, key};
    let scratch = Scratch::new("recall-prime");
    let (index, _) = RecallIndex::open(&place(&scratch));
    let session =
        miyu_kernel::id::SessionId::parse("0192f3a0-1111-7abc-8def-001122334455").unwrap();
    let mut events = Vec::new();
    for n in 0..1000u64 {
        let first = n * 4 + 2;
        let lines = [
            format!(
                r#"{{"seq":{first},"at":"2026-10-07T08:00:00.000Z","kind":"message.user","by":{{"kind":"person","account":"admin"}},"body":{{"blocks":[{{"type":"text","text":"第 {n} 句：周末想去看樱花，顺便吃个火锅"}}]}}}}"#
            ),
            format!(
                r#"{{"seq":{},"at":"2026-10-07T08:00:00.000Z","kind":"turn.started","turn":{},"by":{{"kind":"kernel"}},"body":{{"trigger":{first}}}}}"#,
                first + 1,
                first + 1
            ),
            format!(
                r#"{{"seq":{},"at":"2026-10-07T08:00:00.000Z","kind":"message.assistant","turn":{},"by":{{"kind":"model","endpoint":"deepseek","model":"deepseek-v4"}},"body":{{"blocks":[{{"type":"text","text":"好呀，第 {n} 次去看樱花，火锅要辣的还是清汤的？"}}],"seen":{}}}}}"#,
                first + 2,
                first + 1,
                first + 1
            ),
            format!(
                r#"{{"seq":{},"at":"2026-10-07T08:00:00.000Z","kind":"turn.ended","turn":{},"by":{{"kind":"kernel"}},"body":{{"reason":"completed"}}}}"#,
                first + 3,
                first + 1
            ),
        ];
        events.extend(lines.iter().map(|line| Event::from_line(line).unwrap()));
    }
    let started = std::time::Instant::now();
    let (_, changes) = TurnFeed::primed(&events, None);
    let primed = started.elapsed();
    let edits: Vec<Edit> = changes
        .into_iter()
        .filter_map(|change| match change {
            Change::Put(item) => Some(Edit::Put {
                key: key(&session, item.turn),
                text: item.text,
                at: item.at,
            }),
            _ => None,
        })
        .collect();
    assert_eq!(edits.len(), 1000);
    let started = std::time::Instant::now();
    index
        .apply(&session.to_string(), &edits, events.last().unwrap().seq)
        .unwrap();
    let applied = started.elapsed();
    println!("prime 4000 events: {primed:?}, apply 1000 turns: {applied:?}");
}

#[test]
fn tombstones_are_buried_and_unburied_with_a_batch() {
    let scratch = Scratch::new("recall-buried");
    let (index, _) = RecallIndex::open(&place(&scratch));
    assert!(!index.is_buried("s1/3").unwrap());
    index
        .apply(
            "s1",
            &[
                Edit::Bury { key: "s1/3".into() },
                Edit::Bury { key: "s1/8".into() },
            ],
            seq(9),
        )
        .unwrap();
    assert!(index.is_buried("s1/3").unwrap() && index.is_buried("s1/8").unwrap());
    index
        .apply("s1", &[Edit::Unbury { key: "s1/3".into() }], seq(10))
        .unwrap();
    assert!(!index.is_buried("s1/3").unwrap());
    assert!(index.is_buried("s1/8").unwrap(), "揭的只是那一块");
    // 埋两次、揭没埋的都不碍事。
    index
        .apply(
            "s1",
            &[
                Edit::Bury { key: "s1/8".into() },
                Edit::Unbury { key: "s9/1".into() },
            ],
            seq(11),
        )
        .unwrap();
    assert!(index.is_buried("s1/8").unwrap());
}

#[test]
fn a_source_is_alive_unless_its_turn_or_its_session_is_buried() {
    use miyu_kernel::id::{SessionId, TurnId};
    use miyu_recall::Source;
    use miyu_store::recall::RecallIndexes;
    let scratch = Scratch::new("recall-alive");
    let root = root_in(&scratch);
    let indexes = RecallIndexes::new(&root);
    let session = SessionId::parse("0192f3a0-1111-7abc-8def-001122334455").unwrap();
    let other = SessionId::parse("0192f3a0-2222-7abc-8def-001122334455").unwrap();
    let at_turn = |session: &SessionId, n: u64| Source {
        session: session.clone(),
        turn: TurnId::new(seq(n)),
    };
    let (turns, _) = indexes.turns(&Room::persona(&admin(), "engineer"));
    turns
        .apply(
            &session.to_string(),
            &[Edit::Bury {
                key: format!("{session}/3"),
            }],
            seq(4),
        )
        .unwrap();
    assert!(
        !indexes
            .alive(&Room::persona(&admin(), "engineer"), &at_turn(&session, 3))
            .unwrap(),
        "撤销了的那一轮"
    );
    assert!(
        indexes
            .alive(&Room::persona(&admin(), "engineer"), &at_turn(&session, 8))
            .unwrap()
    );
    indexes.forget_session(&admin(), &other).unwrap();
    assert!(
        !indexes
            .alive(&Room::persona(&admin(), "engineer"), &at_turn(&other, 8))
            .unwrap(),
        "删掉了的会话"
    );
    assert!(
        indexes
            .alive(&Room::persona(&admin(), "miyu"), &at_turn(&session, 3))
            .unwrap(),
        "别的人格的回合库里没埋"
    );
}
