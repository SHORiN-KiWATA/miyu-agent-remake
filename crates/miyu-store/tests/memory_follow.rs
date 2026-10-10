//! 跟着看记忆日志（施工 R-12 上，`docs/blueprint/memory.md`「协议」的流 `memory`）：交回 `after` 以后的、到哪为止，之后追加的
//! 交给回调，连同这时的底账；回调交回 `false` 的不再交；不写 `after` 的不补；补的和交的不丢不重。

use std::sync::{Arc, Mutex};

use miyu_kernel::event::Event;
use miyu_kernel::origin::By;
use miyu_recall::{MemoryBook, MemoryEvent, Saved};
use miyu_store::memory::{Follower, MemoryLogs};
use miyu_store::recall::Room;

use crate::support::*;

fn person() -> By {
    serde_json::from_str(r#"{"kind":"person","account":"admin"}"#).expect("合写法")
}

fn saved(text: &str) -> MemoryEvent {
    MemoryEvent::Saved(Saved {
        class: "user".into(),
        text: text.into(),
        sources: Vec::new(),
        audience: vec![person()],
        replaces: None,
        about: None,
    })
}

/// 交过的：序号、交的时候底账里有没有这一条。
type Seen = Arc<Mutex<Vec<(u64, bool)>>>;

/// 记下交给它的序号、交的时候底账里有没有这一条；交满 `keep` 次以后交回 `false`。
fn recorder(keep: usize) -> (Follower, Seen) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let kept = Arc::clone(&seen);
    let follower: Follower = Box::new(move |event: &Event, book: &MemoryBook| {
        let known = book.get(miyu_recall::MemoryId::new(event.seq)).is_some();
        let mut seen = kept.lock().expect("没崩");
        seen.push((event.seq.get(), known));
        seen.len() < keep
    });
    (follower, seen)
}

#[test]
fn after_fills_in_and_later_ones_are_handed_over() {
    let scratch = Scratch::new("memory-follow");
    let root = root_in(&scratch);
    let (log, _) = MemoryLogs::new(&root)
        .open(&Room::persona(&admin(), "engineer"))
        .expect("开得了");
    let (follower, seen) = recorder(usize::MAX);
    let (filled, upto) = log.follow(Some(0), follower).expect("读得出");
    assert!(filled.is_empty());
    assert_eq!(upto, 0, "一条都没有的是 0");
    for text in ["一", "二", "三"] {
        log.append(at(1), person(), None, &saved(text))
            .expect("记得下");
    }
    assert_eq!(
        *seen.lock().expect("没崩"),
        [(1, true), (2, true), (3, true)],
        "落了盘、算进底账以后交"
    );

    let (follower, later) = recorder(usize::MAX);
    let (filled, upto) = log.follow(Some(1), follower).expect("读得出");
    let seqs: Vec<u64> = filled.iter().map(|event| event.seq.get()).collect();
    assert_eq!((seqs, upto), (vec![2, 3], 3), "补 1 以后的");
    let (follower, quiet) = recorder(usize::MAX);
    let (filled, upto) = log.follow(None, follower).expect("读得出");
    assert!(filled.is_empty(), "不写 after 的不补");
    assert_eq!(upto, 3);
    let (follower, _) = recorder(usize::MAX);
    assert!(
        log.follow(Some(3), follower).expect("读得出").0.is_empty(),
        "跟上了的不补"
    );
    log.append(at(2), person(), None, &saved("四"))
        .expect("记得下");
    assert_eq!(
        *later.lock().expect("没崩"),
        [(4, true)],
        "之后的接着交，不重"
    );
    assert_eq!(*quiet.lock().expect("没崩"), [(4, true)]);
}

#[test]
fn a_follower_that_says_no_is_dropped() {
    let scratch = Scratch::new("memory-follow-drop");
    let root = root_in(&scratch);
    let (log, _) = MemoryLogs::new(&root)
        .open(&Room::persona(&admin(), "engineer"))
        .expect("开得了");
    let (follower, seen) = recorder(1);
    log.follow(None, follower).expect("读得出");
    for text in ["一", "二"] {
        log.append(at(1), person(), None, &saved(text))
            .expect("记得下");
    }
    assert_eq!(
        *seen.lock().expect("没崩"),
        [(1, true)],
        "交回 false 以后不再交"
    );
}
