//! 记忆事件的格和底账（`memory.md`「对外的样子」的记忆日志、底账）。

use miyu_kernel::event::Event;
use miyu_kernel::id::{Seq, SessionId, TurnId};
use miyu_kernel::origin::By;
use miyu_kernel::time::Timestamp;

use super::*;

mod merged;

fn seq(n: u64) -> Seq {
    Seq::new(n).expect("序号从 1 起")
}

fn at(minutes: i64) -> Timestamp {
    let start = Timestamp::parse("2026-10-07T08:00:00.000Z").expect("合写法");
    Timestamp::from_unix_millis(start.unix_millis() + minutes * 60_000).expect("合写法")
}

fn admin() -> By {
    serde_json::from_str(r#"{"kind":"person","account":"admin"}"#).expect("合写法")
}

fn tool() -> By {
    serde_json::from_str(r#"{"kind":"tool","call_id":"call_3_1"}"#).expect("合写法")
}

fn source(turn: u64) -> Source {
    Source {
        session: SessionId::parse("0192f3a0-1111-7abc-8def-001122334455").expect("合写法"),
        turn: TurnId::new(seq(turn)),
    }
}

fn saved(text: &str, replaces: Option<MemoryId>) -> Saved {
    Saved {
        class: "user".to_string(),
        text: text.to_string(),
        sources: vec![source(3)],
        audience: vec![admin()],
        replaces,
        about: None,
    }
}

#[test]
fn a_saved_memory_round_trips_as_an_event() {
    let mut body = saved("用 N 卡", None);
    body.about = Some("2026-10".to_string());
    let event = to_event(seq(7), at(1), tool(), &MemoryEvent::Saved(body.clone())).expect("写得出");
    assert_eq!(event.body.kind(), "ext.memory.saved");
    assert_eq!(
        event.to_line(),
        r#"{"seq":7,"at":"2026-10-07T08:01:00.000Z","kind":"ext.memory.saved","by":{"kind":"tool","call_id":"call_3_1"},"body":{"class":"user","text":"用 N 卡","sources":[{"session":"0192f3a0-1111-7abc-8def-001122334455","turn":3}],"audience":[{"kind":"person","account":"admin"}],"about":"2026-10"}}"#
    );
    let back = Event::from_line(&event.to_line()).expect("读得回");
    assert_eq!(from_event(&back), Some(Ok(MemoryEvent::Saved(body)))); // 没有的不写：不出 `"about":null`、`"replaces":null`。
    let bare = to_event(
        seq(8),
        at(1),
        tool(),
        &MemoryEvent::Saved(saved("养了一只猫", None)),
    )
    .expect("写得出");
    assert!(
        bare.to_line()
            .ends_with(r#""audience":[{"kind":"person","account":"admin"}]}}"#),
        "{}",
        bare.to_line()
    );
}

#[test]
fn a_retirement_round_trips_and_other_events_are_not_memory() {
    let body = Retired {
        id: MemoryId::new(seq(7)),
        why: "user asked".to_string(),
    };
    let event =
        to_event(seq(8), at(2), admin(), &MemoryEvent::Retired(body.clone())).expect("写得出");
    assert!(
        event
            .to_line()
            .contains(r#""body":{"id":"m7","why":"user asked"}"#),
        "{}",
        event.to_line()
    );
    assert_eq!(from_event(&event), Some(Ok(MemoryEvent::Retired(body))));
    let other = Event::from_line(
        r#"{"seq":1,"at":"2026-10-07T08:00:00.000Z","kind":"ext.weather.read","by":{"kind":"kernel"},"body":{}}"#,
    )
    .expect("读得出");
    assert_eq!(from_event(&other), None);
    let broken = Event::from_line(
        r#"{"seq":1,"at":"2026-10-07T08:00:00.000Z","kind":"ext.memory.saved","by":{"kind":"kernel"},"body":{"text":3}}"#,
    )
    .expect("外壳读得出");
    assert!(matches!(from_event(&broken), Some(Err(_))));
}

#[test]
fn an_unknown_class_is_kept_as_it_is() {
    let mut body = saved("喜欢猫", None);
    body.class = "mood".to_string();
    let event =
        to_event(seq(2), at(0), admin(), &MemoryEvent::Saved(body.clone())).expect("写得出");
    assert_eq!(
        from_event(&Event::from_line(&event.to_line()).expect("读得回")),
        Some(Ok(MemoryEvent::Saved(body)))
    );
    assert!(CLASSES.contains(&"feedback") && !CLASSES.contains(&"mood"));
}

#[test]
fn a_memory_id_is_m_and_the_sequence_number() {
    assert_eq!(MemoryId::new(seq(42)).to_string(), "m42");
    assert_eq!(MemoryId::parse("m42"), Some(MemoryId::new(seq(42))));
    for bad in ["42", "m0", "m", "mx", "M42", "m-1", "m+5", "m 5"] {
        assert_eq!(MemoryId::parse(bad), None, "{bad}");
    }
}

#[test]
fn the_book_follows_saves_replacements_and_retirements() {
    let mut book = MemoryBook::default();
    let events = [
        to_event(
            seq(1),
            at(0),
            tool(),
            &MemoryEvent::Saved(saved("用 A 卡", None)),
        ),
        to_event(
            seq(2),
            at(1),
            tool(),
            &MemoryEvent::Saved(saved("养了一只猫", None)),
        ),
        to_event(
            seq(3),
            at(2),
            tool(),
            &MemoryEvent::Saved(saved("用 N 卡", Some(MemoryId::new(seq(1))))),
        ),
        to_event(
            seq(4),
            at(3),
            admin(),
            &MemoryEvent::Retired(Retired {
                id: MemoryId::new(seq(2)),
                why: "不养了".into(),
            }),
        ),
        // 改一条、作废一条不存在的：记下，不碍事。
        to_event(
            seq(5),
            at(4),
            tool(),
            &MemoryEvent::Saved(saved("x", Some(MemoryId::new(seq(99))))),
        ),
        to_event(
            seq(6),
            at(5),
            admin(),
            &MemoryEvent::Retired(Retired {
                id: MemoryId::new(seq(98)),
                why: "?".into(),
            }),
        ),
    ];
    for event in events {
        book.see(&event.expect("写得出")).expect("读得懂");
    }
    let first = book.get(MemoryId::new(seq(1))).expect("在");
    assert_eq!(first.replaced_by, Some(MemoryId::new(seq(3))));
    assert!(!first.current());
    let cat = book.get(MemoryId::new(seq(2))).expect("在");
    assert_eq!(cat.retired.as_deref(), Some("不养了"));
    assert!(!cat.current());
    let now = book.get(MemoryId::new(seq(3))).expect("在");
    assert!(now.current());
    assert_eq!(
        (now.text.as_str(), now.at, now.by.clone()),
        ("用 N 卡", at(2), tool())
    );
    assert_eq!(
        book.all()
            .map(|entry| entry.id.to_string())
            .collect::<Vec<_>>(),
        ["m1", "m2", "m3", "m5"]
    );
}

fn from(session: &str, turn: u64) -> Source {
    Source {
        session: SessionId::parse(session).expect("合写法"),
        turn: TurnId::new(seq(turn)),
    }
}

const S: &str = "0192f3a0-1111-7abc-8def-001122334455";
const T: &str = "0192f3a0-2222-7abc-8def-001122334455";

/// 清空（施工 R-3 补，`memory.md` 第二条第 5 款）：带会话的、整间的都写成事件、读回一字不差。
#[test]
fn a_clearing_round_trips_with_or_without_a_session() {
    for (body, json) in [
        (Cleared { session: None }, r#""body":{}}"#),
        (
            Cleared {
                session: Some(SessionId::parse(S).expect("合写法")),
            },
            r#""body":{"session":"0192f3a0-1111-7abc-8def-001122334455"}}"#,
        ),
    ] {
        let event =
            to_event(seq(9), at(1), admin(), &MemoryEvent::Cleared(body.clone())).expect("写得出");
        assert_eq!(event.body.kind(), "ext.memory.cleared");
        assert!(event.to_line().ends_with(json), "{}", event.to_line());
        let back = Event::from_line(&event.to_line()).expect("读得回");
        assert_eq!(from_event(&back), Some(Ok(MemoryEvent::Cleared(body))));
    }
}

/// 清掉一个会话的：出处全在它里面的才清，还有别的出处的、人记的不动；整间的：那以前的全清，以后记的照常。清掉的不算数。
#[test]
fn clearing_takes_what_came_only_from_the_session_or_everything_before() {
    let mut book = MemoryBook::default();
    let save = |n: u64, sources: Vec<Source>| {
        let mut body = saved(&format!("第 {n} 条"), None);
        body.sources = sources;
        to_event(seq(n), at(n as i64), tool(), &MemoryEvent::Saved(body)).expect("写得出")
    };
    let clear = |n: u64, session: Option<&str>| {
        let body = Cleared {
            session: session.map(|session| SessionId::parse(session).expect("合写法")),
        };
        (
            body.clone(),
            to_event(seq(n), at(n as i64), admin(), &MemoryEvent::Cleared(body)).expect("写得出"),
        )
    };
    let ids = |list: Vec<MemoryId>| list.iter().map(ToString::to_string).collect::<Vec<_>>();
    for event in [
        save(1, vec![from(S, 3)]),
        save(2, vec![from(S, 3), from(T, 5)]),
        save(3, Vec::new()),
        save(4, vec![from(T, 7)]),
        save(5, vec![from(S, 4), from(S, 9)]),
    ] {
        book.see(&event).expect("读得懂");
    }
    let (body, event) = clear(6, Some(S));
    assert_eq!(ids(book.clears(&body)), ["m1", "m5"]);
    book.see(&event).expect("读得懂");
    let current = |book: &MemoryBook| {
        book.all()
            .filter(|entry| entry.current())
            .map(|entry| entry.id.to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(current(&book), ["m2", "m3", "m4"]);
    assert!(book.get(MemoryId::new(seq(1))).expect("在").cleared);
    assert_eq!(
        ids(book.clears(&body)),
        Vec::<String>::new(),
        "清过的不再算"
    );

    book.see(&save(7, vec![from(S, 12)])).expect("读得懂");
    let (body, event) = clear(8, None);
    assert_eq!(ids(book.clears(&body)), ["m2", "m3", "m4", "m7"]);
    book.see(&event).expect("读得懂");
    book.see(&save(9, vec![from(S, 13)])).expect("读得懂");
    assert_eq!(current(&book), ["m9"], "清空以后记的照常");
}

/// 人经协议、斜杠命令写的带着命令编号（`cause`，04 第六节第 1 条）：底账记下每个编号做成了什么——哪一条、清掉几条，同一个
/// 编号再发照它回答，核心重启以后照日志算回来。
#[test]
fn the_book_remembers_what_each_command_did() {
    use miyu_kernel::id::CommandId;
    let cause = |text: &str| CommandId::parse(text).expect("合写法");
    let mut book = MemoryBook::default();
    let mut save = to_event(
        seq(1),
        at(1),
        admin(),
        &MemoryEvent::Saved(saved("一", None)),
    )
    .expect("写得出");
    save.cause = Some(cause("c1"));
    book.see(&save).expect("读得懂");
    let untold = to_event(
        seq(2),
        at(2),
        tool(),
        &MemoryEvent::Saved(saved("二", None)),
    )
    .expect("写得出");
    book.see(&untold).expect("读得懂");
    let mut clear = to_event(
        seq(3),
        at(3),
        admin(),
        &MemoryEvent::Cleared(Cleared { session: None }),
    )
    .expect("写得出");
    clear.cause = Some(cause("c2"));
    book.see(&clear).expect("读得懂");
    assert_eq!(book.done(&cause("c1")), Some((MemoryId::new(seq(1)), 0)));
    assert_eq!(book.done(&cause("c2")), Some((MemoryId::new(seq(3)), 2)));
    assert_eq!(book.done(&cause("c3")), None);
}

/// 清空交回的条数不算改掉的旧版本：人看得见的才算（施工 R-3 补）。
#[test]
fn clearing_counts_what_could_be_seen() {
    let mut book = MemoryBook::default();
    for event in [
        to_event(
            seq(1),
            at(1),
            tool(),
            &MemoryEvent::Saved(saved("用 A 卡", None)),
        ),
        to_event(
            seq(2),
            at(2),
            tool(),
            &MemoryEvent::Saved(saved("用 N 卡", Some(MemoryId::new(seq(1))))),
        ),
        to_event(
            seq(3),
            at(3),
            admin(),
            &MemoryEvent::Cleared(Cleared { session: None }),
        ),
    ] {
        let event = event.expect("写得出");
        let cleared = book.see(&event).expect("读得懂");
        assert_eq!(cleared, usize::from(event.seq == seq(3)));
    }
    assert!(book.all().all(|entry| entry.cleared), "旧版本也标成清掉");
}

/// 抽到哪（施工 R-6 上）：`ext.memory.extracted` 读写对得上，跳过的原因写成小写的字，没跳的不写；底账照最大的 `upto` 记。
#[test]
fn an_extraction_mark_round_trips_and_the_book_keeps_the_furthest() {
    let session = SessionId::parse("0192f3a0-1111-7abc-8def-001122334455").expect("合写法");
    let mark = |upto: u64, skipped| {
        MemoryEvent::Extracted(Extracted {
            session: session.clone(),
            upto: seq(upto),
            count: 2,
            skipped,
        })
    };
    let event =
        to_event(seq(3), at(1), admin(), &mark(9, Some(Skipped::Remembered))).expect("写得出");
    assert!(
        event.to_line().ends_with(r#""body":{"session":"0192f3a0-1111-7abc-8def-001122334455","upto":9,"count":2,"skipped":"remembered"}}"#),
        "{}",
        event.to_line()
    );
    let back = Event::from_line(&event.to_line()).expect("读得回");
    assert_eq!(
        from_event(&back),
        Some(Ok(mark(9, Some(Skipped::Remembered))))
    );
    let plain = to_event(seq(4), at(1), admin(), &mark(5, None)).expect("写得出");
    assert!(
        plain.to_line().ends_with(r#""count":2}}"#),
        "没跳的不写 skipped"
    );
    let mut book = MemoryBook::default();
    for event in [&event, &plain] {
        book.see(event).expect("读得懂");
    }
    assert_eq!(
        book.extracted(&session),
        Some(seq(9)),
        "后来写的小的不往回退"
    );
    assert_eq!(book.all().count(), 0);
}
