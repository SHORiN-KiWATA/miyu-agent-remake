//! 人要的两种单开的那一轮：手动压缩（施工 6-8）、清空（施工 6-8 补）。没有触发的回合开始照收，别的回合的规矩照查；摘要是
//! 空的只许清空。

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

/// 摘要是空的只许清空（施工 6-8 补）：别的几种、没写原因的（当作到线了）都拦下；清空的收。
#[test]
fn only_a_clear_has_an_empty_summary() {
    let mut ledger = after(18);
    for trigger in [
        "",
        r#","trigger":"auto""#,
        r#","trigger":"manual""#,
        r#","trigger":"overflow""#,
        r#","trigger":"scheduled""#,
    ] {
        let body = format!(r#"{{"upto":16,"summary":""{trigger}}}"#);
        refused(
            &mut ledger,
            &event(19, Some(18), "context.compacted", &body),
            "the summary is empty; only a clear has an empty summary",
        );
    }
    ledger
        .append(&event(
            19,
            Some(18),
            "context.compacted",
            r#"{"upto":16,"summary":"","trigger":"clear"}"#,
        ))
        .unwrap();
    // 摘要不空的清空照收：账本只查空的那一头。
    let mut ledger = after(18);
    ledger
        .append(&event(
            19,
            Some(18),
            "context.compacted",
            r#"{"upto":16,"summary":"S","trigger":"clear"}"#,
        ))
        .unwrap();
}
