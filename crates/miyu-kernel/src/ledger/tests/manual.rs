//! 手动压缩单开的那一轮没有触发（施工 6-8）：照收，别的回合的规矩照查。

use super::*;

#[test]
fn a_turn_without_a_trigger_starts_too() {
    // 前 16 条：第二轮撤掉了，没有回合在进行。
    let mut ledger = after(16);
    ledger
        .append(&event(17, Some(17), "turn.started", "{}"))
        .unwrap();
    refused(
        &mut ledger,
        &event(18, Some(18), "turn.started", "{}"),
        "turn 17 has not ended",
    );
}
