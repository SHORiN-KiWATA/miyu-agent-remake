//! 撤销与恢复的规矩（02 第九节）：压缩以前的回合也能撤，撤的范围里的压缩不再算数，恢复了跟着回来（施工 6-9）；
//! 空闲时才撤；从某一轮起往后一轮不漏；撤过的不再撤。恢复的正好是最近一次撤销的那几轮；下一轮开始以后，不能恢复。
//! 合规的会话里，回合 3 在 3 到 9，回合 11 在 10 到 14，15 号撤掉回合 11，回合 18 一开头（19 号）压缩到 16。

use super::*;

fn reverted(seq: u64, turns: &str) -> Event {
    event(
        seq,
        None,
        "turn.reverted",
        &format!(r#"{{"turns":{turns}}}"#),
    )
}

fn unreverted(seq: u64, turns: &str) -> Event {
    event(
        seq,
        None,
        "turn.unreverted",
        &format!(r#"{{"turns":{turns}}}"#),
    )
}

fn seq(n: u64) -> Seq {
    Seq::new(n).unwrap()
}

/// 回合编号，照 `turn.started` 的序号。
fn turns(seqs: &[u64]) -> Vec<TurnId> {
    seqs.iter().map(|&n| TurnId::new(seq(n))).collect()
}

/// 合规的会话走完，回合 18 结束；再开一轮 22，它一开头（23 号）压缩到 20，结束在 24 号。
fn two_compactions() -> Ledger {
    let mut ledger = after(19);
    for event in [
        event(20, Some(18), "turn.ended", r#"{"reason":"completed"}"#),
        event(21, None, "message.user", SAID),
        event(22, Some(22), "turn.started", r#"{"trigger":21}"#),
        event(
            23,
            Some(22),
            "context.compacted",
            r#"{"upto":20,"summary":"…"}"#,
        ),
        event(24, Some(22), "turn.ended", r#"{"reason":"completed"}"#),
    ] {
        ledger.append(&event).unwrap();
    }
    ledger
}

#[test]
fn turns_before_a_compaction_can_be_undone_and_take_it_along() {
    let mut ledger = after(14);
    refused(
        &mut ledger,
        &reverted(15, "[11,12]"),
        "turn 12 is not in the current history: no such turn, or already undone",
    );
    // 两次压缩替代到 20，回合 3 也照样能撤：撤的范围里的两次压缩都不再算数。
    let mut ledger = two_compactions();
    assert_eq!(ledger.compacted(), Some(seq(20)));
    assert_eq!(ledger.turns_from(turns(&[3])[0]), Some(turns(&[3, 18, 22])));
    refused(
        &mut ledger,
        &reverted(25, "[11,18,22]"),
        "turn 11 is not in the current history",
    );
    ledger.append(&reverted(25, "[3,18,22]")).unwrap();
    assert_eq!(ledger.compacted(), None);
    assert_eq!(ledger.last_turn(), None);
    // 恢复了，两次压缩跟着回来。
    ledger.append(&unreverted(26, "[3,18,22]")).unwrap();
    assert_eq!(ledger.compacted(), Some(seq(20)));
    // 只撤后一轮，前一次压缩还算数：撤掉以后再压，可以比撤掉的那一次早。
    ledger.append(&reverted(27, "[22]")).unwrap();
    assert_eq!(ledger.compacted(), Some(seq(16)));
    for event in [
        event(28, None, "message.user", SAID),
        event(29, Some(29), "turn.started", r#"{"trigger":28}"#),
        event(
            30,
            Some(29),
            "context.compacted",
            r#"{"upto":17,"summary":"…"}"#,
        ),
    ] {
        ledger.append(&event).unwrap();
    }
    assert_eq!(ledger.compacted(), Some(seq(17)));
}

/// 从哪一条读回（施工 6-9）：撤完以后还算数的最近一次压缩替代到的下一条，一次都没有的是第 1 条；撤不到压缩的没有。
#[test]
fn read_back_starts_after_the_compaction_that_still_counts() {
    let mut ledger = two_compactions();
    let from = |ledger: &Ledger, turn: u64| ledger.read_back_from(turns(&[turn])[0]);
    assert_eq!(from(&ledger, 3), Some(Seq::FIRST));
    assert_eq!(from(&ledger, 18), Some(Seq::FIRST));
    assert_eq!(from(&ledger, 22), Some(seq(17)));
    for event in [
        event(25, None, "message.user", SAID),
        event(26, Some(26), "turn.started", r#"{"trigger":25}"#),
        event(27, Some(26), "turn.ended", r#"{"reason":"completed"}"#),
    ] {
        ledger.append(&event).unwrap();
    }
    assert_eq!(from(&ledger, 26), None, "撤不到压缩的，照以前在内存里撤");
    // 撤掉回合 22，它的压缩不算了：再撤 18，从第 1 条读回。
    ledger.append(&reverted(28, "[22,26]")).unwrap();
    assert_eq!(from(&ledger, 18), Some(Seq::FIRST));
}

#[test]
fn revert_takes_a_turn_and_every_one_after_it() {
    let mut ledger = after(14);
    assert_eq!(ledger.turns_from(turns(&[3])[0]), Some(turns(&[3, 11])));
    // 中间的一轮不能单独撤，后面的要照先后一轮不漏。
    refused(
        &mut ledger,
        &reverted(15, "[3]"),
        "undo every turn from 3 on, in order: 3, 11",
    );
    refused(
        &mut ledger,
        &reverted(15, "[11,3]"),
        "undo every turn from 11 on, in order: 11",
    );
    refused(
        &mut ledger,
        &reverted(15, "[]"),
        "the list of undone turns is empty",
    );
    ledger.append(&reverted(15, "[3,11]")).unwrap();
    // 撤过的不再撤。
    refused(
        &mut ledger,
        &reverted(16, "[11]"),
        "turn 11 is not in the current history",
    );
    assert_eq!(ledger.turns_from(turns(&[3])[0]), None);
}

#[test]
fn no_revert_while_a_turn_is_running() {
    let mut ledger = after(11);
    refused(
        &mut ledger,
        &reverted(12, "[3]"),
        "turn 11 is still running; nothing can be undone",
    );
}

#[test]
fn unrevert_brings_back_the_latest_revert_only() {
    let mut ledger = after(14);
    refused(&mut ledger, &unreverted(15, "[11]"), "nothing to redo");
    ledger.append(&reverted(15, "[11]")).unwrap();
    ledger.append(&reverted(16, "[3]")).unwrap();
    assert_eq!(ledger.last_reverted(), Some(turns(&[3]).as_slice()));
    refused(
        &mut ledger,
        &unreverted(17, "[11]"),
        "redo the turns of the latest undo: 3",
    );
    ledger.append(&unreverted(17, "[3]")).unwrap();
    ledger.append(&unreverted(18, "[11]")).unwrap();
    refused(&mut ledger, &unreverted(19, "[11]"), "nothing to redo");
    // 恢复了的回到有效历史里，又能撤。
    assert_eq!(ledger.turns_from(turns(&[3])[0]), Some(turns(&[3, 11])));
    ledger.append(&reverted(19, "[11]")).unwrap();
}

#[test]
fn no_unrevert_after_the_next_turn_or_a_compaction() {
    // 15 号撤掉回合 11；撤了以后开了下一轮，不能恢复。
    let ledger = after(15);
    assert_eq!(ledger.last_reverted(), Some(turns(&[11]).as_slice()));
    let mut next = after(18);
    assert_eq!(next.last_reverted(), None);
    refused(&mut next, &unreverted(19, "[11]"), "nothing to redo");
    // 压缩在回合里，那一轮开的时候就不能恢复了；压完也一样。
    let mut compacted = after(19);
    refused(&mut compacted, &unreverted(20, "[11]"), "nothing to redo");
}

/// 改回文件的结局（施工 4-7 上）只在回合外：撤销、恢复都在空闲时。
#[test]
fn files_are_restored_only_between_turns() {
    let files = r#"{"files":[]}"#;
    let mut ledger = after(3);
    refused(
        &mut ledger,
        &event(4, None, "files.restored", files),
        "files are restored only after an undo or a redo",
    );
    let mut ledger = after(15);
    ledger
        .append(&event(16, None, "files.restored", files))
        .expect("撤销以后，回合外记得下");
}
