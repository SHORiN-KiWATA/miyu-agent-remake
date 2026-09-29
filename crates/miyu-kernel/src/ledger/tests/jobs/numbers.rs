//! 用过的最大任务编号（施工 7-5）：执行器照它往下领号；照最后一段数，带前缀的和不带的混在一起也不重（施工 7-1 补）。
//! 底子见 `jobs.rs`。

use super::*;

/// 用过的最大编号（施工 7-5）：执行器照它往下领号。一个都没派过的是 0；不认识的种类也占着号；撤掉那一轮，号照样算用过。
#[test]
fn the_last_job_number_counts_every_job_ever_started() {
    assert_eq!(Ledger::default().last_job_number(), 0);
    assert_eq!(jobs_after(4).last_job_number(), 0, "还没派");
    assert_eq!(jobs_after(5).last_job_number(), 1);
    assert_eq!(jobs_after(9).last_job_number(), 3, "不认识的种类也算");
    let mut ledger = jobs_after(9);
    ledger
        .append(&event(10, None, "turn.reverted", r#"{"turns":[3]}"#))
        .unwrap();
    assert_eq!(ledger.last_job_number(), 3, "撤掉的回合里派的也算");
}

/// 子会话派的带着前缀（施工 7-1 补）：以前的日志里派过 `j1`、`j2`、`j3`（底子）、`j7`，这以后派的是 `j5.8`。编号照整个比，
/// `j5.8` 和 `j7`、`j5` 都不是同一个，重了的才拦；用过的最大编号照最后一段数：`j5.8` 照整个编号排在 `j7` 前面，数的却是 8。
#[test]
fn prefixed_job_ids_count_by_their_last_part() {
    let mut ledger = jobs_after(9);
    for (seq, turn, kind, body) in [
        (10, None, "message.user", SAID.to_string()),
        (
            11,
            Some(11),
            "turn.started",
            r#"{"trigger":10}"#.to_string(),
        ),
        (12, Some(11), "message.assistant", reply(12, 11, 3, false)),
    ] {
        ledger.append(&event(seq, turn, kind, &body)).unwrap();
    }
    let started_in = |seq, call: &str, effects: &[String]| {
        event(seq, Some(11), "tool.result", &result_with(call, effects))
    };
    ledger
        .append(&started_in(
            13,
            "call_12_1",
            &[started("j7", "command", None)],
        ))
        .unwrap();
    assert_eq!(ledger.last_job_number(), 7);
    let prefixed = [
        started("j5.8", "command", None),
        started("j5", "agent", Some(B)),
    ];
    ledger
        .append(&started_in(14, "call_12_2", &prefixed))
        .unwrap();
    assert_eq!(ledger.last_job_number(), 8, "照最后一段数");
    refused(
        &mut ledger,
        &started_in(15, "call_12_3", &[started("j5.8", "command", None)]),
        "job j5.8 is already taken",
    );
    ledger
        .append(&started_in(
            15,
            "call_12_3",
            &[started("j5.8.1", "command", None)],
        ))
        .unwrap();
    assert_eq!(ledger.last_job_number(), 8, "j5.8.1 数的是 1");
    ledger
        .append(&event(
            16,
            None,
            "job.reported",
            &job_reported("j5.8", "exited"),
        ))
        .unwrap();
    refused(
        &mut ledger,
        &event(17, None, "job.reported", &job_reported("j5.8", "exited")),
        "job j5.8 has already ended",
    );
}
