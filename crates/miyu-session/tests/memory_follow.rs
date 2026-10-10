//! 跟着看一间的记忆日志（施工 R-12 上，`docs/blueprint/memory.md`「协议」的流 `memory`）：照听众挑，听不到的那一条和作废它的
//! 不给，整间的（抽到哪、合并、摘要、清空）照给；补的和之后的一个挑法；清掉的哪里都不出来：补的时候清掉的那几条、清空以前的
//! 摘要不给，出处全死了的也不给；攒满了算掉队、不再交。

use std::sync::atomic::Ordering;

use miyu_kernel::id::{ExternalId, Seq, SessionId, TurnId, VenueId};
use miyu_kernel::origin::{By, External};
use miyu_recall::{Cleared, MemoryEvent, MemoryId, Merged, Retired, Saved, Source, Summary};
use miyu_session::Keeper;

use crate::support::meaning::{persona, save};
use crate::support::*;

/// 群里的一个人：alice 听不到只给他的。
fn stranger() -> By {
    By::External(External {
        venue: VenueId::parse("qq:group:1").expect("合写法"),
        id: ExternalId::parse("qq:10001").expect("合写法"),
        account: None,
        role: None,
    })
}

fn append(home: &Home, by: By, event: MemoryEvent) {
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    log.append(now(), by, None, &event).expect("记得下");
}

fn seqs(events: &[miyu_kernel::event::Event]) -> Vec<u64> {
    events.iter().map(|event| event.seq.get()).collect()
}

#[tokio::test]
async fn only_what_the_hearer_may_hear_and_the_whole_room() {
    let home = Home::new();
    save(&home, "用户养了一只猫");
    append(
        &home,
        stranger(),
        MemoryEvent::Saved(Saved {
            class: "user".into(),
            text: "只有群里那个人听得到的".into(),
            sources: Vec::new(),
            audience: vec![stranger()],
            replaces: None,
            about: None,
        }),
    );
    append(
        &home,
        stranger(),
        MemoryEvent::Retired(Retired {
            id: MemoryId::parse("m2").expect("合写法"),
            why: "不要了".into(),
        }),
    );
    let merged = || {
        MemoryEvent::Merged(Merged {
            upto: miyu_kernel::id::Seq::new(1).expect("从 1 起"),
            given: 1,
            revised: 0,
            retired: 0,
            failed: false,
        })
    };
    append(&home, alice(), merged());
    let keeper = Keeper::new(&home.memory, persona(), vec![alice()]);
    let mut following = keeper.follow(Some(0), 8).expect("跟得上");
    assert_eq!(
        seqs(&following.filled),
        [1, 4],
        "听不到的那一条和作废它的不补，整间的照补"
    );
    assert_eq!(following.upto, 4);

    save(&home, "用户住在杭州");
    append(
        &home,
        stranger(),
        MemoryEvent::Saved(Saved {
            class: "user".into(),
            text: "又一条听不到的".into(),
            sources: Vec::new(),
            audience: vec![stranger()],
            replaces: None,
            about: None,
        }),
    );
    append(&home, alice(), merged());
    let mut live = Vec::new();
    while let Ok(event) = following.live.try_recv() {
        live.push(event);
    }
    assert_eq!(seqs(&live), [5, 7], "之后的一个挑法");
    assert!(!following.lagged.load(Ordering::SeqCst));
}

#[tokio::test]
async fn a_full_queue_is_a_lag_and_stops() {
    let home = Home::new();
    let keeper = Keeper::new(&home.memory, persona(), vec![alice()]);
    let mut following = keeper.follow(None, 1).expect("跟得上");
    for text in ["一", "二", "三"] {
        save(&home, text);
    }
    assert_eq!(
        following.live.recv().await.map(|event| event.seq.get()),
        Some(1)
    );
    assert_eq!(following.live.recv().await, None, "放不进的那一刻起不再交");
    assert!(following.lagged.load(Ordering::SeqCst), "是掉了队");
}

fn summary(text: &str, upto: u64) -> MemoryEvent {
    MemoryEvent::Summary(Summary {
        text: text.into(),
        upto: Seq::new(upto).expect("从 1 起"),
    })
}

fn cleared(session: Option<SessionId>) -> MemoryEvent {
    MemoryEvent::Cleared(Cleared { session })
}

#[tokio::test]
async fn what_was_cleared_is_not_filled_in() {
    let home = Home::new();
    save(&home, "用户养了一只猫");
    append(&home, alice(), summary("用户养猫", 1));
    append(&home, alice(), cleared(None));
    save(&home, "用户住在杭州");
    append(&home, alice(), summary("用户住在杭州", 4));
    let keeper = Keeper::new(&home.memory, persona(), vec![alice()]);
    for after in [0, 1] {
        let following = keeper.follow(Some(after), 8).expect("跟得上");
        assert_eq!(
            seqs(&following.filled),
            [3, 4, 5],
            "清掉的那一条、清空以前的摘要不补；清空、之后的照补（after {after}）"
        );
    }
    let mut following = keeper.follow(None, 8).expect("跟得上");
    append(&home, alice(), cleared(None));
    let live = following.live.try_recv().expect("推了");
    assert_eq!(live.seq.get(), 6, "之后的清空照推，头照它重新列");
}

#[tokio::test]
async fn a_cleared_session_hides_only_what_came_from_it() {
    let home = Home::new();
    let parse = |text: &str| SessionId::parse(text).expect("合写法");
    let (cleared_one, other) = (
        parse("0192f3a0-1111-7abc-8def-001122334455"),
        parse("0192f3a0-3333-7abc-8def-001122334455"),
    );
    // 两个会话都在：出处活着，挑掉的只因为清了。
    for session in [&cleared_one, &other] {
        std::fs::create_dir_all(home.root.session_dir(&alice_account(), session)).expect("建得了");
    }
    let from = |session: &SessionId| {
        MemoryEvent::Saved(Saved {
            class: "user".into(),
            text: "从那个会话来的".into(),
            sources: vec![Source {
                session: session.clone(),
                turn: TurnId::new(Seq::FIRST),
            }],
            audience: vec![alice()],
            replaces: None,
            about: None,
        })
    };
    append(&home, alice(), from(&cleared_one));
    append(&home, alice(), from(&other));
    save(&home, "人记的");
    append(
        &home,
        alice(),
        MemoryEvent::Retired(Retired {
            id: MemoryId::parse("m1").expect("合写法"),
            why: "不对".into(),
        }),
    );
    append(&home, alice(), cleared(Some(cleared_one)));
    let keeper = Keeper::new(&home.memory, persona(), vec![alice()]);
    let following = keeper.follow(Some(0), 8).expect("跟得上");
    assert_eq!(
        seqs(&following.filled),
        [2, 3, 5],
        "清掉的那一条和作废它的不补；别的会话来的、人记的、清空照补"
    );
}

#[tokio::test]
async fn a_memory_whose_sources_are_gone_is_not_filled_in() {
    let home = Home::new();
    // 这个会话的目录不在（删了、清出回收处）：出处死了。
    let gone = SessionId::parse("0192f3a0-2222-7abc-8def-001122334455").expect("合写法");
    append(
        &home,
        alice(),
        MemoryEvent::Saved(Saved {
            class: "user".into(),
            text: "从删掉的会话来的".into(),
            sources: vec![Source {
                session: gone,
                turn: TurnId::new(Seq::FIRST),
            }],
            audience: vec![alice()],
            replaces: None,
            about: None,
        }),
    );
    save(&home, "人记的");
    append(
        &home,
        alice(),
        MemoryEvent::Retired(Retired {
            id: MemoryId::parse("m1").expect("合写法"),
            why: "不对".into(),
        }),
    );
    let keeper = Keeper::new(&home.memory, persona(), vec![alice()]);
    let following = keeper.follow(Some(0), 8).expect("跟得上");
    assert_eq!(
        seqs(&following.filled),
        [2],
        "出处全死了的那一条和作废它的当它不在"
    );
}
