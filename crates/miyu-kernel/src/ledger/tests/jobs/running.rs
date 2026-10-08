//! 还在跑的任务的样子（施工 9-6 上）：种类、子会话照账本，标题照派它的那一条；报了结束的后台命令、回报过的子代理、不认识的
//! 种类不在里面。

use super::*;

/// 账本交出的还在跑的任务，写成 `job.started` 的样子。
fn running(ledger: &Ledger) -> Vec<String> {
    ledger
        .running_started()
        .iter()
        .map(|started| serde_json::to_string(started).unwrap())
        .collect()
}

#[test]
fn running_jobs_carry_what_their_job_started_said() {
    let mut ledger = jobs_after(9);
    assert_eq!(
        running(&ledger),
        [
            r#"{"job":"j1","what":"command","title":"t"}"#.to_string(),
            format!(r#"{{"job":"j2","what":"agent","title":"t","session":"{A}"}}"#),
        ],
        "不认识的种类 j3 不在"
    );
    ledger
        .append(&event(
            10,
            None,
            "job.reported",
            &job_reported("j1", "exited"),
        ))
        .unwrap();
    ledger
        .append(&event_by(
            11,
            None,
            "child.reported",
            &session(A),
            &child_reported("j2", A, "done"),
        ))
        .unwrap();
    assert!(running(&ledger).is_empty(), "报完了都不在");
    assert_eq!(
        ledger.running_jobs(),
        Vec::<JobId>::new(),
        "和 running_jobs 是同一批"
    );
}
