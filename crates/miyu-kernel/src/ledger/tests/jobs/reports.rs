//! 两种回报的几条（施工 7-1）：后台命令只报一次结束；子代理的回报对得上它的会话和 `by`，可以报好几次，停掉的不再报；
//! 回合进行中到的带上正在进行的那一轮。底子见 `jobs.rs`。

use super::*;

#[test]
fn a_background_command_ends_once() {
    let mut ledger = jobs_after(9);
    for (job, why) in [
        (
            "j2",
            "job j2 is not a background command: no such job, or it is not a command",
        ),
        ("j3", "job j3 is not a background command"),
        ("j9", "job j9 is not a background command"),
    ] {
        refused(
            &mut ledger,
            &event(10, None, "job.reported", &job_reported(job, "exited")),
            why,
        );
    }
    ledger
        .append(&event(
            10,
            None,
            "job.reported",
            &job_reported("j1", "exited"),
        ))
        .unwrap();
    refused(
        &mut ledger,
        &event(11, None, "job.reported", &job_reported("j1", "exited")),
        "job j1 has already ended",
    );
    // 不认识的原因也算结束。
    let mut ledger = jobs_after(9);
    ledger
        .append(&event(
            10,
            None,
            "job.reported",
            &job_reported("j1", "evicted"),
        ))
        .unwrap();
    refused(
        &mut ledger,
        &event(11, None, "job.reported", &job_reported("j1", "stopped")),
        "job j1 has already ended",
    );
}

#[test]
fn a_child_reports_as_its_own_session() {
    let mut ledger = jobs_after(9);
    for (job, why) in [
        (
            "j1",
            "job j1 is not a subagent: no such job, or it is not an agent",
        ),
        ("j3", "job j3 is not a subagent"),
        ("j9", "job j9 is not a subagent"),
    ] {
        refused(
            &mut ledger,
            &event_by(
                10,
                None,
                "child.reported",
                &session(A),
                &child_reported(job, A, "done"),
            ),
            why,
        );
    }
    refused(
        &mut ledger,
        &event_by(
            10,
            None,
            "child.reported",
            &session(B),
            &child_reported("j2", B, "done"),
        ),
        &format!("job j2 runs in session {A}, not {B}"),
    );
    for by in [
        session(B),
        r#"{"kind":"kernel"}"#.to_string(),
        r#"{"kind":"person","account":"alice"}"#.to_string(),
    ] {
        refused(
            &mut ledger,
            &event_by(
                10,
                None,
                "child.reported",
                &by,
                &child_reported("j2", A, "done"),
            ),
            &format!("child.reported for job j2 should be by session {A}"),
        );
    }
    ledger
        .append(&event_by(
            10,
            None,
            "child.reported",
            &session(A),
            &child_reported("j2", A, "done"),
        ))
        .unwrap();
}

/// 以 stopped、undone 报过的不再报：被停掉的不会再起来。不认识的原因不拦。
#[test]
fn a_stopped_child_reports_no_more() {
    for last in ["stopped", "undone"] {
        let mut ledger = jobs_after(9);
        ledger
            .append(&event_by(
                10,
                None,
                "child.reported",
                &session(A),
                &child_reported("j2", A, last),
            ))
            .unwrap();
        refused(
            &mut ledger,
            &event_by(
                11,
                None,
                "child.reported",
                &session(A),
                &child_reported("j2", A, "done"),
            ),
            "job j2 was stopped or undone and cannot report again",
        );
    }
    let mut ledger = jobs_after(9);
    ledger
        .append(&event_by(
            10,
            None,
            "child.reported",
            &session(A),
            &child_reported("j2", A, "handed_off"),
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
}

/// 回合进行中到的带上正在进行的那一轮，闲着到的不带：照「带 `turn` 的是正在进行的那个回合」查。
#[test]
fn reports_in_a_turn_carry_the_running_turn() {
    let mut ledger = jobs_after(6);
    refused(
        &mut ledger,
        &event(7, Some(2), "job.reported", &job_reported("j1", "exited")),
        "turn 2 is not the running turn",
    );
    ledger
        .append(&event(
            7,
            Some(3),
            "job.reported",
            &job_reported("j1", "exited"),
        ))
        .unwrap();
    ledger
        .append(&event_by(
            8,
            Some(3),
            "child.reported",
            &session(A),
            &child_reported("j2", A, "done"),
        ))
        .unwrap();
    // 回合进行中不带也收：带不带由写它的一方照规矩定，账本只查带着的。
    ledger
        .append(&event_by(
            9,
            None,
            "child.reported",
            &session(A),
            &child_reported("j2", A, "done"),
        ))
        .unwrap();
    let mut ledger = jobs_after(9);
    refused(
        &mut ledger,
        &event_by(
            10,
            Some(3),
            "child.reported",
            &session(A),
            &child_reported("j2", A, "done"),
        ),
        "turn 3 is not the running turn",
    );
}
