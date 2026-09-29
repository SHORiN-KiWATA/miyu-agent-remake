//! 留着一切的那一份（`History::whole()`，施工 6-4）：压缩替代掉的不丢，摘要自己也是一条；撤销、恢复、撤回和
//! 有效历史照同一套规矩算。`history` 照它算哪些还算数。

use super::*;

/// 同 [`feed`]，交给留着一切的那一份。
fn feed_whole(events: impl IntoIterator<Item = Event>) -> History {
    let mut ledger = Ledger::default();
    let mut history = History::whole();
    for event in events {
        ledger.append(&event).unwrap();
        history.append(event);
    }
    history
}

#[test]
fn what_compaction_replaced_stays_and_the_summary_is_an_entry() {
    let mut events = two_turns();
    events.push(compacted(10, 9));
    events.push(message(11, ALICE));
    let whole = feed_whole(events.clone());
    assert_eq!(seqs(&whole), (1..=11).collect::<Vec<_>>());
    assert_eq!(checkpoint(&whole), None);
    // 同一份日志，有效历史只剩压缩以后的。
    assert_eq!(seqs(&feed(events)), [11]);
}

#[test]
fn undo_and_redo_count_the_same_as_in_the_effective_history() {
    let mut events = two_turns();
    events.push(reverted(10, &[7]));
    assert_eq!(
        seqs(&feed_whole(events.clone())),
        seqs(&feed(events.clone()))
    );
    assert_eq!(seqs(&feed_whole(events.clone())), [1, 2, 3, 4, 5]);
    events.push(event(
        11,
        None,
        ALICE,
        "turn.unreverted",
        r#"{"turns":[7]}"#,
    ));
    assert_eq!(seqs(&feed_whole(events)), (1..=9).collect::<Vec<_>>());
}

#[test]
fn undoing_a_turn_after_a_compaction_leaves_the_earlier_part_alone() {
    let mut events = two_turns();
    events.push(compacted(10, 9));
    events.push(message(11, ALICE));
    events.extend(turn(12, 11));
    events.push(reverted(15, &[12]));
    assert_eq!(seqs(&feed_whole(events)), (1..=10).collect::<Vec<_>>());
}

#[test]
fn withdrawn_messages_are_gone_here_too() {
    let mut events = two_turns();
    events.push(compacted(10, 9));
    events.push(message(11, ALICE));
    events.extend([
        event(12, Some(12), KERNEL, "turn.started", r#"{"trigger":11}"#),
        event(
            13,
            Some(12),
            KERNEL,
            "model.called",
            r#"{"seen":12,"messages":1,"result":"interrupted"}"#,
        ),
        event(14, Some(12), ALICE, "message.user", r#"{"blocks":[]}"#),
        event(
            15,
            Some(12),
            ALICE,
            "message.withdrawn",
            r#"{"messages":[14]}"#,
        ),
        event(
            16,
            Some(12),
            ALICE,
            "turn.ended",
            r#"{"reason":"interrupted"}"#,
        ),
    ]);
    assert_eq!(
        seqs(&feed_whole(events)),
        [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 16]
    );
}
