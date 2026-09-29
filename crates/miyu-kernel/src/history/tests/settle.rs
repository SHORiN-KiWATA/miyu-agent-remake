//! 落到检查点上、从日志的一段重建（施工 6-9，`docs/blueprint/kernel/history.md`）：最近的压缩当检查点，比它早的一起
//! 丢，原文清掉，没有压缩的不动；撤掉的回合里的压缩放在一边，恢复放回来再落；没有撤掉过压缩的日志，重建的和一条条
//! 收的一样。

use super::*;
use crate::id::ContentHash;

/// 从第 `from` 条起留着一切地收，再落到检查点上：载入、撤掉压缩时就这样重建。
fn rebuilt(events: &[Event], from: u64) -> History {
    let mut history = History::whole();
    for event in events.iter().filter(|event| event.seq.get() >= from) {
        history.append(event.clone());
    }
    history.settle();
    history
}

/// 两次压缩：第一次单开一轮（10 到 12）替代到 5，问一句（13）、一轮（14 到 16），第二次单开一轮（17 到 19）替代到 13。
fn compacted_twice() -> Vec<Event> {
    let mut events = two_turns();
    events.extend(compaction_turn(10, 9, 5));
    events.push(message(13, ALICE));
    events.extend(turn(14, 13));
    events.extend(compaction_turn(17, 16, 13));
    events
}

#[test]
fn the_latest_compaction_becomes_the_checkpoint_and_the_earlier_ones_go() {
    let events = compacted_twice();
    let mut history = History::whole();
    for event in events {
        history.append(event);
    }
    history.recall(BTreeMap::from([(ContentHash::of(b"x"), "x".to_string())]));
    assert!(history.settle(), "换了检查点");
    assert_eq!(checkpoint(&history), Some(18));
    assert_eq!(seqs(&history), vec![14, 15, 16, 17, 19]);
    assert_eq!(history.recalled(&ContentHash::of(b"x")), None, "原文清掉");
    // 平时那一份事件里没有压缩：再落一次，什么都不动。
    let before = history.clone();
    assert!(!history.settle());
    assert_eq!(history, before);
}

#[test]
fn rebuilt_from_after_the_counting_checkpoint_is_the_same_as_fed_one_by_one() {
    // 没有撤掉过压缩：撤、恢复、再撤压缩以后的一轮，撤掉的放在一边。
    let mut events = two_turns();
    events.extend(compaction_turn(10, 9, 5));
    events.push(message(13, ALICE));
    events.extend(turn(14, 13));
    events.push(reverted(17, &[14]));
    events.push(event(
        18,
        None,
        ALICE,
        "turn.unreverted",
        r#"{"turns":[14]}"#,
    ));
    events.push(reverted(19, &[14]));
    let fed = feed(events.clone());
    // 还算数的最近一次压缩替代到 5：从 6 起。
    let rebuilt = rebuilt(&events, 6);
    assert_eq!(checkpoint(&fed), Some(11));
    assert_eq!(checkpoint(&rebuilt), checkpoint(&fed));
    assert_eq!(rebuilt.events(), fed.events());
    assert_eq!(rebuilt.last_undone(), fed.last_undone());
    assert!(!fed.last_undone().is_empty());
}

#[test]
fn undone_compactions_are_put_aside_and_come_back_on_redo() {
    let mut events = compacted_twice();
    events.push(reverted(20, &[10, 14, 17]));
    let mut history = rebuilt(&events, 1);
    assert_eq!(checkpoint(&history), None, "两次压缩都撤掉了，从头");
    assert_eq!(seqs(&history), (1..=9).collect::<Vec<_>>());
    let undone: Vec<u64> = history
        .last_undone()
        .iter()
        .map(|event| event.seq.get())
        .collect();
    assert_eq!(undone, (10..=19).collect::<Vec<_>>(), "压缩也放在一边");
    history.append(event(
        21,
        None,
        ALICE,
        "turn.unreverted",
        r#"{"turns":[10,14,17]}"#,
    ));
    assert_eq!(checkpoint(&history), Some(18), "恢复了，落到最近的那一次上");
    assert_eq!(seqs(&history), vec![14, 15, 16, 17, 19]);
}

#[test]
fn the_whole_one_keeps_a_redone_compaction_as_an_entry() {
    let mut events = two_turns();
    events.extend(compaction_turn(10, 9, 10));
    events.push(reverted(13, &[10]));
    events.push(event(
        14,
        None,
        ALICE,
        "turn.unreverted",
        r#"{"turns":[10]}"#,
    ));
    let mut ledger = Ledger::default();
    let mut whole = History::whole();
    for event in events {
        ledger.append(&event).unwrap();
        whole.append(event);
    }
    assert_eq!(checkpoint(&whole), None);
    assert_eq!(seqs(&whole), (1..=12).collect::<Vec<_>>());
}
