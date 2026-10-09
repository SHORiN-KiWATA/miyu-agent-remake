//! 合并的两种事件和底账记的那几样（施工 R-7 上，`memory.md` 第七条第 4 款）：摘要、合到哪的记号读写得回；底账照最后一份
//! 摘要、最后一次合并，数那以后抽过几个会话；一条记得它改的是哪一条。

use super::*;

#[test]
fn a_summary_and_a_merge_mark_round_trip() {
    let summary = MemoryEvent::Summary(Summary {
        text: "用户住在东京。".into(),
        upto: seq(4),
    });
    let event = to_event(seq(5), at(1), admin(), &summary).expect("写得出");
    assert!(
        event
            .to_line()
            .ends_with(r#""body":{"text":"用户住在东京。","upto":4}}"#),
        "{}",
        event.to_line()
    );
    let back = Event::from_line(&event.to_line()).expect("读得回");
    assert_eq!(from_event(&back), Some(Ok(summary)));
    let merged = |failed| {
        MemoryEvent::Merged(Merged {
            upto: seq(4),
            given: 3,
            revised: 1,
            retired: 1,
            failed,
        })
    };
    let ok = to_event(seq(6), at(1), admin(), &merged(false)).expect("写得出");
    assert!(
        ok.to_line()
            .ends_with(r#""body":{"upto":4,"given":3,"revised":1,"retired":1}}"#),
        "没失败的不写 failed：{}",
        ok.to_line()
    );
    let failed = to_event(seq(7), at(1), admin(), &merged(true)).expect("写得出");
    assert!(failed.to_line().ends_with(r#""failed":true}}"#));
    for event in [ok, failed] {
        let back = Event::from_line(&event.to_line()).expect("读得回");
        assert!(matches!(
            from_event(&back),
            Some(Ok(MemoryEvent::Merged(_)))
        ));
    }
}

#[test]
fn the_book_keeps_the_last_summary_and_merge_and_counts_sessions_since() {
    let session = |n: u64| {
        SessionId::parse(&format!("0192f3a0-1111-7abc-8def-00112233445{n}")).expect("合写法")
    };
    let extracted = |n: u64| {
        MemoryEvent::Extracted(Extracted {
            session: session(n),
            upto: seq(9),
            count: 1,
            skipped: None,
        })
    };
    let summary = |text: &str| {
        MemoryEvent::Summary(Summary {
            text: text.into(),
            upto: seq(2),
        })
    };
    let merged = MemoryEvent::Merged(Merged {
        upto: seq(2),
        given: 2,
        revised: 0,
        retired: 0,
        failed: false,
    });
    let mut book = MemoryBook::default();
    let mut next = 0;
    let mut see = |book: &mut MemoryBook, event: &MemoryEvent, minutes: i64| {
        next += 1;
        book.see(&to_event(seq(next), at(minutes), admin(), event).expect("写得出"))
            .expect("读得懂");
    };
    assert_eq!((book.summary(), book.merged()), (None, None));
    for n in [1, 2, 1] {
        see(&mut book, &extracted(n), 0);
    }
    assert_eq!(
        book.sessions_since_merge(),
        2,
        "没合并过的照全部，同一个会话算一个"
    );
    see(&mut book, &summary("旧的"), 1);
    see(&mut book, &summary("用户住在东京。"), 1);
    see(&mut book, &merged, 2);
    assert_eq!(book.summary(), Some("用户住在东京。"), "照最后一份");
    assert_eq!(book.merged(), Some((at(2), seq(2))));
    assert_eq!(book.sessions_since_merge(), 0, "合并以后重新数");
    see(&mut book, &extracted(3), 3);
    see(&mut book, &extracted(3), 3);
    assert_eq!(book.sessions_since_merge(), 1);
    assert_eq!(book.all().count(), 0, "摘要、记号都不是一条记忆");
}

#[test]
fn an_entry_knows_what_it_replaces() {
    let mut book = MemoryBook::default();
    let first = to_event(
        seq(1),
        at(0),
        admin(),
        &MemoryEvent::Saved(saved("旧", None)),
    )
    .expect("写得出");
    let second = to_event(
        seq(2),
        at(1),
        admin(),
        &MemoryEvent::Saved(saved("新", Some(MemoryId::new(seq(1))))),
    )
    .expect("写得出");
    for event in [&first, &second] {
        book.see(event).expect("读得懂");
    }
    assert_eq!(book.get(MemoryId::new(seq(1))).expect("在").replaces, None);
    assert_eq!(
        book.get(MemoryId::new(seq(2))).expect("在").replaces,
        Some(MemoryId::new(seq(1)))
    );
}
