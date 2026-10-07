//! 记忆事件的格和底账（`memory.md`「对外的样子」的记忆日志、底账）。

use miyu_kernel::event::Event;
use miyu_kernel::id::{Seq, SessionId, TurnId};
use miyu_kernel::origin::By;
use miyu_kernel::time::Timestamp;

use super::*;

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
