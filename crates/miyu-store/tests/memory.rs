//! 记忆日志的登记（施工 R-3 上，`docs/blueprint/memory.md`「对外的样子」的记忆日志、底账、记忆库）：记了、重开还在；最后
//! 半行坏了截掉；几个线程同时记，序号连续不重；记忆库删了、落后了，打开时照日志补。

use std::fs;
use std::sync::Arc;

use miyu_kernel::id::{Seq, SessionId, TurnId};
use miyu_kernel::origin::By;
use miyu_recall::{Cleared, MemoryEvent, MemoryId, Retired, Saved, Source};
use miyu_store::memory::MemoryLogs;
use miyu_store::recall::Room;

use crate::support::*;

fn tool() -> By {
    serde_json::from_str(r#"{"kind":"tool","call_id":"call_3_1"}"#).expect("合写法")
}

fn saved(text: &str) -> MemoryEvent {
    MemoryEvent::Saved(Saved {
        class: "user".into(),
        text: text.into(),
        sources: vec![Source {
            session: SessionId::parse("0192f3a0-1111-7abc-8def-001122334455").expect("合写法"),
            turn: TurnId::new(Seq::new(3).expect("从 1 起")),
        }],
        audience: vec![
            serde_json::from_str(r#"{"kind":"person","account":"admin"}"#).expect("合写法"),
        ],
        replaces: None,
        about: None,
    })
}

fn id(n: u64) -> MemoryId {
    MemoryId::new(Seq::new(n).expect("从 1 起"))
}

#[test]
fn what_is_saved_is_there_after_reopening_and_found_by_words() {
    let scratch = Scratch::new("memory-reopen");
    let root = root_in(&scratch);
    let logs = MemoryLogs::new(&root);
    let log = logs
        .open(&Room::persona(&admin(), "engineer"))
        .expect("开得了")
        .0;
    assert_eq!(
        log.append(at(1), tool(), None, &saved("用户用 N 卡"))
            .expect("记得下")
            .id,
        id(1)
    );
    assert_eq!(
        log.append(at(2), tool(), None, &saved("周末喜欢爬山"))
            .expect("记得下")
            .id,
        id(2)
    );
    let retired = MemoryEvent::Retired(Retired {
        id: id(2),
        why: "说错了".into(),
    });
    assert_eq!(
        log.append(at(3), tool(), None, &retired)
            .expect("记得下")
            .id,
        id(3)
    );
    drop((log, logs));
    assert!(
        root.account_dir(&admin())
            .join("modules/memory/engineer")
            .is_dir()
    );

    let logs = MemoryLogs::new(&root);
    let log = logs
        .open(&Room::persona(&admin(), "engineer"))
        .expect("开得了")
        .0;
    let texts: Vec<String> = log.book(|book| book.all().map(|entry| entry.text.clone()).collect());
    assert_eq!(texts, ["用户用 N 卡", "周末喜欢爬山"]);
    assert!(log.book(|book| book.get(id(2)).expect("在").retired.is_some()));
    assert_eq!(log.search("显卡 N卡", 10).expect("搜得了"), [id(1)]);
    assert_eq!(
        log.search("爬山", 10).expect("搜得了"),
        [id(2)],
        "作废的照样搜得到，挑不挑是用的一方的事"
    );
    assert!(
        Arc::ptr_eq(
            &log,
            &logs
                .open(&Room::persona(&admin(), "engineer"))
                .expect("开得了")
                .0
        ),
        "一份一个"
    );
}

#[test]
fn a_torn_last_line_is_cut_off() {
    let scratch = Scratch::new("memory-torn");
    let root = root_in(&scratch);
    let logs = MemoryLogs::new(&root);
    logs.open(&Room::persona(&admin(), "engineer"))
        .expect("开得了")
        .0
        .append(at(1), tool(), None, &saved("养了一只猫"))
        .expect("记得下");
    drop(logs);
    let segment = root
        .account_dir(&admin())
        .join("modules/memory/engineer/000000000001.jsonl");
    let mut bytes = fs::read(&segment).expect("在");
    bytes.extend_from_slice(br#"{"seq":2,"at":"2026-10"#);
    fs::write(&segment, bytes).expect("写得进");
    let logs = MemoryLogs::new(&root);
    let log = logs
        .open(&Room::persona(&admin(), "engineer"))
        .expect("截掉半行照样开")
        .0;
    assert_eq!(
        log.append(at(2), tool(), None, &saved("猫叫团子"))
            .expect("记得下")
            .id,
        id(2),
        "半行不算，接着它写"
    );
    assert_eq!(log.book(|book| book.all().count()), 2);
}

#[test]
fn saves_from_several_threads_get_distinct_consecutive_numbers() {
    let scratch = Scratch::new("memory-threads");
    let root = root_in(&scratch);
    let logs = Arc::new(MemoryLogs::new(&root));
    let threads: Vec<_> = (0..8)
        .map(|t| {
            let logs = Arc::clone(&logs);
            std::thread::spawn(move || {
                let log = logs
                    .open(&Room::persona(&admin(), "engineer"))
                    .expect("开得了")
                    .0;
                (0..25)
                    .map(|n| {
                        log.append(
                            at(n),
                            tool(),
                            None,
                            &saved(&format!("第 {t} 个线程的第 {n} 条")),
                        )
                        .expect("记得下")
                        .id
                    })
                    .collect::<Vec<_>>()
            })
        })
        .collect();
    let mut ids: Vec<MemoryId> = threads
        .into_iter()
        .flat_map(|t| t.join().expect("没崩"))
        .collect();
    ids.sort();
    assert_eq!(ids, (1..=200).map(id).collect::<Vec<_>>());
    let log = logs
        .open(&Room::persona(&admin(), "engineer"))
        .expect("开得了")
        .0;
    assert_eq!(log.book(|book| book.all().count()), 200);
}

#[test]
fn a_missing_or_lagging_memory_index_is_filled_from_the_log() {
    let scratch = Scratch::new("memory-index");
    let root = root_in(&scratch);
    let logs = MemoryLogs::new(&root);
    let log = logs
        .open(&Room::persona(&admin(), "engineer"))
        .expect("开得了")
        .0;
    log.append(at(1), tool(), None, &saved("用户用 N 卡"))
        .expect("记得下");
    log.append(at(2), tool(), None, &saved("喜欢吃火锅"))
        .expect("记得下");
    drop((log, logs));
    let index = root.index(&admin()).join("recall/memory-engineer.db");
    for suffix in ["", "-wal", "-shm"] {
        let mut name = index.clone().into_os_string();
        name.push(suffix);
        match fs::remove_file(name) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => panic!("{error}"),
        }
    }
    let logs = MemoryLogs::new(&root);
    let (log, report) = logs
        .open(&Room::persona(&admin(), "engineer"))
        .expect("开得了");
    assert_eq!(report.expect("第一次开交回情形").filled, 2, "整份补");
    assert_eq!(log.search("火锅", 10).expect("搜得了"), [id(2)]);
    assert_eq!(log.search("N卡", 10).expect("搜得了"), [id(1)]);
    drop((log, logs));
    let (_, report) = MemoryLogs::new(&root)
        .open(&Room::persona(&admin(), "engineer"))
        .expect("开得了");
    assert_eq!(
        report.expect("第一次开交回情形").filled,
        0,
        "跟得上的不再写"
    );
}

#[test]
fn personas_and_accounts_have_their_own_logs() {
    let scratch = Scratch::new("memory-apart");
    let root = root_in(&scratch);
    let logs = MemoryLogs::new(&root);
    logs.open(&Room::persona(&admin(), "engineer"))
        .expect("开得了")
        .0
        .append(at(1), tool(), None, &saved("写代码用 Rust"))
        .expect("记得下");
    let miyu = logs
        .open(&Room::persona(&admin(), "miyu"))
        .expect("开得了")
        .0;
    assert_eq!(
        miyu.append(at(2), tool(), None, &saved("喜欢猫"))
            .expect("记得下")
            .id,
        id(1),
        "另一份从 1 起"
    );
    assert!(miyu.search("Rust", 10).expect("搜得了").is_empty());
}

#[test]
fn the_first_opening_reports_and_an_unreadable_memory_is_named() {
    use miyu_store::recall::Opened;
    let scratch = Scratch::new("memory-report");
    let root = root_in(&scratch);
    let logs = MemoryLogs::new(&root);
    let (log, report) = logs
        .open(&Room::persona(&admin(), "engineer"))
        .expect("开得了");
    let report = report.expect("第一次开交回情形");
    assert!(
        matches!(report.index, Opened::Created),
        "{:?}",
        report.index
    );
    assert!(report.caught_up.is_none() && report.unreadable.is_empty());
    assert!(
        logs.open(&Room::persona(&admin(), "engineer"))
            .expect("开得了")
            .1
            .is_none(),
        "开过的不再交"
    );
    log.append(at(1), tool(), None, &saved("用户用 N 卡"))
        .expect("记得下");
    drop((log, logs));
    // 外壳是好的、记忆的 body 读不懂的一行：底账跳过它、说出来，别的照常。
    let segment = root
        .account_dir(&admin())
        .join("modules/memory/engineer/000000000001.jsonl");
    let mut bytes = fs::read(&segment).expect("在");
    bytes.extend_from_slice(
        b"{\"seq\":2,\"at\":\"2026-10-01T00:02:00.000Z\",\"kind\":\"ext.memory.saved\",\"by\":{\"kind\":\"kernel\"},\"body\":{\"text\":3}}\n",
    );
    fs::write(&segment, bytes).expect("写得进");
    let logs = MemoryLogs::new(&root);
    let (log, report) = logs
        .open(&Room::persona(&admin(), "engineer"))
        .expect("开得了");
    let report = report.expect("第一次开交回情形");
    assert_eq!(report.unreadable.len(), 1, "{:?}", report.unreadable);
    assert_eq!(report.unreadable[0].0, Seq::new(2).expect("从 1 起"));
    assert_eq!(log.book(|book| book.all().count()), 1);
    assert_eq!(
        log.append(at(3), tool(), None, &saved("喜欢猫"))
            .expect("记得下")
            .id,
        id(3)
    );
}

/// 量尺（施工单 R-3 上「风险」第 1 条）：一千条记忆的日志、记忆库多大，开一次（算底账、补记忆库）多久。
/// `cargo test -p miyu-store --release --test memory -- --ignored --nocapture`。
#[test]
#[ignore = "量尺"]
fn measure_a_thousand_memories() {
    let scratch = Scratch::new("memory-measure");
    let root = root_in(&scratch);
    let logs = MemoryLogs::new(&root);
    let (log, _) = logs
        .open(&Room::persona(&admin(), "engineer"))
        .expect("开得了");
    for n in 0..1000 {
        log.append(
            at(n),
            tool(),
            None,
            &saved(&format!(
                "第 {n} 条：用户周末喜欢去爬山，顺便吃个火锅，最近在学 Rust"
            )),
        )
        .expect("记得下");
    }
    drop((log, logs));
    let size = |dir: std::path::PathBuf| -> u64 {
        fs::read_dir(dir)
            .expect("在")
            .map(|entry| entry.expect("读得出").metadata().expect("在").len())
            .sum()
    };
    let logged = size(root.account_dir(&admin()).join("modules/memory/engineer"));
    let started = std::time::Instant::now();
    let logs = MemoryLogs::new(&root);
    let (log, _) = logs
        .open(&Room::persona(&admin(), "engineer"))
        .expect("开得了");
    let opened = started.elapsed();
    assert_eq!(log.book(|book| book.all().count()), 1000);
    println!("log {} KiB, open {opened:?}", logged / 1024);
}

#[test]
fn a_session_room_lives_in_the_session_directory_apart_from_the_persona() {
    use miyu_kernel::id::SessionId;
    use miyu_store::recall::RecallIndexes;
    let scratch = Scratch::new("memory-session-room");
    let root = root_in(&scratch);
    let session = SessionId::parse("0192f3a0-1111-7abc-8def-001122334455").expect("合写法");
    let room = Room::session(&admin(), &session);
    let logs = MemoryLogs::new(&root);
    let (log, _) = logs.open(&room).expect("开得了");
    assert_eq!(
        log.append(at(1), tool(), None, &saved("只在这个会话里记得"))
            .expect("记得下")
            .id,
        id(1)
    );
    let turns = RecallIndexes::new(&root);
    turns
        .turns(&room)
        .0
        .put(&format!("{session}/3"), "只在这个会话里聊过", at(1))
        .expect("放得进");
    let dir = root.session_dir(&admin(), &session).join("memory");
    assert!(
        dir.join("log/000000000001.jsonl").is_file(),
        "记忆日志在会话目录里"
    );
    assert!(
        dir.join("turns.db").is_file() && dir.join("memory.db").is_file(),
        "两个库也在"
    );
    let (persona, _) = logs
        .open(&Room::persona(&admin(), "engineer"))
        .expect("开得了");
    assert!(
        persona.search("会话", 10).expect("搜得了").is_empty(),
        "人格那一间看不到"
    );
    assert!(
        turns
            .turns(&Room::persona(&admin(), "engineer"))
            .0
            .search("聊过", 10)
            .expect("搜得了")
            .is_empty()
    );
    // 删会话照旧只碰人格那几间，会话那一间跟着目录走。
    turns.forget_session(&admin(), &session).expect("拿得掉");
    assert_eq!(turns.turns(&room).0.keys().expect("读得了").len(), 1);
}

/// 清空（施工 R-3 补，`memory.md` 第二条第 5 款）：清掉的那几行从记忆库里删掉，追加交回清掉几条；以后记的照常搜得到；记忆库
/// 删了照日志重建，清掉的也不放回去。
#[test]
fn cleared_memories_leave_the_memory_index_and_stay_out_when_it_is_rebuilt() {
    let scratch = Scratch::new("memory-cleared");
    let root = root_in(&scratch);
    let room = Room::persona(&admin(), "engineer");
    let logs = MemoryLogs::new(&root);
    let log = logs.open(&room).expect("开得了").0;
    log.append(at(1), tool(), None, &saved("用户用 N 卡"))
        .expect("记得下");
    log.append(at(2), tool(), None, &saved("喜欢吃火锅"))
        .expect("记得下");
    let clear = MemoryEvent::Cleared(Cleared { session: None });
    let appended = log.append(at(3), tool(), None, &clear).expect("记得下");
    assert_eq!(appended.cleared, 2);
    assert!(appended.indexed.is_ok());
    assert!(log.search("火锅", 10).expect("搜得了").is_empty());
    log.append(at(4), tool(), None, &saved("周末去爬山"))
        .expect("记得下");
    assert_eq!(log.search("爬山", 10).expect("搜得了"), [id(4)]);
    let again = log.append(at(5), tool(), None, &clear).expect("记得下");
    assert_eq!(again.cleared, 1, "清过的不再算");
    log.append(at(6), tool(), None, &saved("喜欢喝茶"))
        .expect("记得下");
    drop((log, logs));

    let index = root.index(&admin()).join("recall/memory-engineer.db");
    for suffix in ["", "-wal", "-shm"] {
        let mut name = index.clone().into_os_string();
        name.push(suffix);
        match fs::remove_file(name) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => panic!("{error}"),
        }
    }
    let logs = MemoryLogs::new(&root);
    let (log, report) = logs.open(&room).expect("开得了");
    assert_eq!(report.expect("第一次开交回情形").filled, 1, "只补没清掉的");
    for words in ["火锅", "N卡", "爬山"] {
        assert!(log.search(words, 10).expect("搜得了").is_empty(), "{words}");
    }
    assert_eq!(log.search("喝茶", 10).expect("搜得了"), [id(6)]);
}

/// 同一个命令编号再追加只算一次（04 第六节第 1 条）：交回头一次的编号和清掉几条，日志里不多一行；重开照日志认得。
#[test]
fn the_same_command_appends_once_even_after_reopening() {
    use miyu_kernel::id::CommandId;
    let scratch = Scratch::new("memory-cause");
    let root = root_in(&scratch);
    let room = Room::persona(&admin(), "engineer");
    let cause = CommandId::parse("c1").expect("合写法");
    let logs = MemoryLogs::new(&root);
    let log = logs.open(&room).expect("开得了").0;
    let first = log
        .append(at(1), tool(), Some(&cause), &saved("用户用 N 卡"))
        .expect("记得下");
    let again = log
        .append(at(2), tool(), Some(&cause), &saved("用户用 A 卡"))
        .expect("记得下");
    assert_eq!((first.id, again.id), (id(1), id(1)));
    assert_eq!(log.book(|book| book.all().count()), 1, "不多记一条");
    let clear = MemoryEvent::Cleared(Cleared { session: None });
    let cleared = CommandId::parse("c2").expect("合写法");
    assert_eq!(
        log.append(at(3), tool(), Some(&cleared), &clear)
            .expect("记得下")
            .cleared,
        1
    );
    drop((log, logs));
    let logs = MemoryLogs::new(&root);
    let log = logs.open(&room).expect("开得了").0;
    let again = log
        .append(at(4), tool(), Some(&cleared), &clear)
        .expect("记得下");
    assert_eq!((again.id, again.cleared), (id(2), 1), "重开以后照日志认得");
    let next = log
        .append(at(5), tool(), None, &saved("喜欢喝茶"))
        .expect("记得下");
    assert_eq!(next.id, id(3), "日志里只有两行");
}
