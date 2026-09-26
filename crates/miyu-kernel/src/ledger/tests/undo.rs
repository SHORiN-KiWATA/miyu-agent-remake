//! 撤销与恢复的规矩（02 第九节）：只撤最近一次压缩以后的；空闲时才撤；从某一轮起往后一轮不漏；
//! 撤过的不再撤。恢复的正好是最近一次撤销的那几轮；下一轮开始、压缩以后，不能恢复。
//! 合规的会话里，回合 3 在 3 到 9，回合 11 在 10 到 14，15 号撤掉回合 11，17 号压缩到 16。

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

/// 回合编号，照 `turn.started` 的序号。
fn turns(seqs: &[u64]) -> Vec<TurnId> {
    seqs.iter()
        .map(|&n| TurnId::new(Seq::new(n).unwrap()))
        .collect()
}

#[test]
fn revert_only_turns_after_the_latest_compaction() {
    let mut ledger = after(14);
    refused(
        &mut ledger,
        &reverted(15, "[11,12]"),
        "回合 12 不在有效历史里",
    );
    // 压缩替代到 16，回合 3 和 11 都在它之前，写进了摘要，撤不了了。
    let mut ledger = after(17);
    refused(&mut ledger, &reverted(18, "[3]"), "在最近一次压缩之前");
}

#[test]
fn revert_takes_a_turn_and_every_one_after_it() {
    let mut ledger = after(14);
    assert_eq!(ledger.turns_from(turns(&[3])[0]), Some(turns(&[3, 11])));
    // 中间的一轮不能单独撤，后面的要照先后一轮不漏。
    refused(
        &mut ledger,
        &reverted(15, "[3]"),
        "要从回合 3 起往后全撤，照先后：3、11",
    );
    refused(
        &mut ledger,
        &reverted(15, "[11,3]"),
        "要从回合 11 起往后全撤，照先后：11",
    );
    refused(&mut ledger, &reverted(15, "[]"), "撤销的列表是空的");
    ledger.append(&reverted(15, "[3,11]")).unwrap();
    // 撤过的不再撤。
    refused(&mut ledger, &reverted(16, "[11]"), "回合 11 不在有效历史里");
    assert_eq!(ledger.turns_from(turns(&[3])[0]), None);
}

#[test]
fn no_revert_while_a_turn_is_running() {
    let mut ledger = after(11);
    refused(
        &mut ledger,
        &reverted(12, "[3]"),
        "回合 11 还在进行，撤销不了",
    );
}

#[test]
fn unrevert_brings_back_the_latest_revert_only() {
    let mut ledger = after(14);
    refused(&mut ledger, &unreverted(15, "[11]"), "没有能恢复的撤销");
    ledger.append(&reverted(15, "[11]")).unwrap();
    ledger.append(&reverted(16, "[3]")).unwrap();
    assert_eq!(ledger.last_reverted(), Some(turns(&[3]).as_slice()));
    refused(
        &mut ledger,
        &unreverted(17, "[11]"),
        "恢复的应该是最近一次撤销的那几轮：3",
    );
    ledger.append(&unreverted(17, "[3]")).unwrap();
    ledger.append(&unreverted(18, "[11]")).unwrap();
    refused(&mut ledger, &unreverted(19, "[11]"), "没有能恢复的撤销");
    // 恢复了的回到有效历史里，又能撤。
    assert_eq!(ledger.turns_from(turns(&[3])[0]), Some(turns(&[3, 11])));
    ledger.append(&reverted(19, "[11]")).unwrap();
}

#[test]
fn no_unrevert_after_the_next_turn_or_a_compaction() {
    // 15 号撤掉回合 11，17 号压缩：压缩以后不能恢复。
    let mut ledger = after(15);
    assert_eq!(ledger.last_reverted(), Some(turns(&[11]).as_slice()));
    let mut compacted = after(17);
    refused(&mut compacted, &unreverted(18, "[11]"), "没有能恢复的撤销");
    // 撤了以后开了下一轮，也不能恢复。
    ledger
        .append(&event(16, None, "message.user", SAID))
        .unwrap();
    ledger
        .append(&event(17, Some(17), "turn.started", r#"{"trigger":16}"#))
        .unwrap();
    assert_eq!(ledger.last_reverted(), None);
}
