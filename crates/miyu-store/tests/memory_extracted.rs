//! 抽到哪（施工 R-6 上，`docs/blueprint/memory.md` 第六条）：记忆日志里的 `ext.memory.extracted` 照会话记最大的那个 `upto`，
//! 重开还在；它不是一条记忆，底账、记忆库里都没有它。

use miyu_kernel::id::{Seq, SessionId};
use miyu_kernel::origin::By;
use miyu_recall::{Extracted, MemoryEvent, Skipped};
use miyu_store::memory::MemoryLogs;
use miyu_store::recall::Room;

use crate::support::*;

fn module() -> By {
    serde_json::from_str(r#"{"kind":"module","id":"memory"}"#).expect("合写法")
}

fn session(n: u8) -> SessionId {
    SessionId::parse(&format!("0192f3a0-1111-7abc-8def-0011223344{n:02}")).expect("合写法")
}

fn extracted(of: u8, upto: u64, skipped: Option<Skipped>) -> MemoryEvent {
    MemoryEvent::Extracted(Extracted {
        session: session(of),
        upto: Seq::new(upto).expect("从 1 起"),
        count: 0,
        skipped,
    })
}

#[test]
fn the_furthest_extraction_of_each_session_is_kept_across_reopening() {
    let scratch = Scratch::new("memory-extracted");
    let root = root_in(&scratch);
    let room = Room::persona(&admin(), "engineer");
    {
        let (log, _) = MemoryLogs::new(&root).open(&room).expect("开得了");
        assert_eq!(log.book(|book| book.extracted(&session(1))), None);
        for event in [
            extracted(1, 9, None),
            extracted(1, 5, Some(Skipped::Failed)),
            extracted(2, 3, Some(Skipped::Remembered)),
        ] {
            log.append(at(1), module(), None, &event).expect("记得下");
        }
        assert_eq!(
            log.book(|book| book.extracted(&session(1))),
            Seq::new(9),
            "照最大的记"
        );
    }
    let (log, _) = MemoryLogs::new(&root).open(&room).expect("重开得了");
    assert_eq!(log.book(|book| book.extracted(&session(1))), Seq::new(9));
    assert_eq!(log.book(|book| book.extracted(&session(2))), Seq::new(3));
    assert_eq!(log.book(|book| book.all().count()), 0, "不是记忆");
    assert!(
        log.index().keys().expect("读得了").is_empty(),
        "记忆库里没有它"
    );
}
