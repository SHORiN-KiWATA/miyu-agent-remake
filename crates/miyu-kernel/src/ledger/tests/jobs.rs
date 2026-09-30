//! 任务的几条规矩（施工 7-1，`kernel/history.md`「账本查的规矩」）：每一条一个被拦下的例子、一个放行的例子。
//!
//! 底子是一段会话：第 3 轮一次调了三个工具，第一个派了后台命令 `j1`，第二个派了子代理 `j2`（会话 A），第三个派了
//! 一个不认识的种类 `j3`；然后回合结束，闲着。两种回报的几条在 `jobs/reports.rs`。

use super::*;

mod reports;

/// 子代理 `j2` 的会话。
const A: &str = "01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91";
/// 另一个会话。
const B: &str = "01a0d78c-ca52-7d19-8b64-0e3f5a7c2d92";
/// 父会话：子会话的 `session.created` 里写它。
const PARENT: &str = "01a0d75d-2180-7a3c-9e41-5b7d2c8f6a10";

/// 拼一条事件，`by` 照写。
fn event_by(seq: u64, turn: Option<u64>, kind: &str, by: &str, body: &str) -> Event {
    let turn = turn.map(|t| format!(r#""turn":{t},"#)).unwrap_or_default();
    Event::from_line(&format!(
        r#"{{"seq":{seq},"at":"2026-09-25T07:00:00.000Z","kind":"{kind}",{turn}"by":{by},"body":{body}}}"#
    ))
    .unwrap()
}

/// 会话 `id` 做的 `by`。
fn session(id: &str) -> String {
    format!(r#"{{"kind":"session","id":"{id}"}}"#)
}

/// 一条成功的工具结果，带着几样效果。
fn result_with(call: &str, effects: &[String]) -> String {
    format!(
        r#"{{"call_id":"{call}","status":"ok","blocks":[],"effects":[{}]}}"#,
        effects.join(",")
    )
}

/// 效果 `job.started`：`session` 有的写上。
fn started(job: &str, what: &str, session: Option<&str>) -> String {
    let session = session
        .map(|s| format!(r#","session":"{s}""#))
        .unwrap_or_default();
    format!(r#"{{"kind":"job.started","job":"{job}","what":"{what}","title":"t"{session}}}"#)
}

/// `job.reported` 的 `body`。
fn job_reported(job: &str, reason: &str) -> String {
    format!(r#"{{"job":"{job}","reason":"{reason}"}}"#)
}

/// `child.reported` 的 `body`。
fn child_reported(job: &str, session: &str, reason: &str) -> String {
    format!(r#"{{"job":"{job}","session":"{session}","reason":"{reason}","text":"好了"}}"#)
}

/// 底子：前 9 条，第 3 轮派了 `j1`、`j2`、`j3`，结束了。
fn with_jobs() -> Vec<Event> {
    vec![
        event(1, None, "session.created", CREATED),
        event(2, None, "message.user", SAID),
        event(3, Some(3), "turn.started", r#"{"trigger":2}"#),
        event(4, Some(3), "message.assistant", &reply(4, 3, 3, false)),
        event(
            5,
            Some(3),
            "tool.result",
            &result_with("call_4_1", &[started("j1", "command", None)]),
        ),
        event(
            6,
            Some(3),
            "tool.result",
            &result_with("call_4_2", &[started("j2", "agent", Some(A))]),
        ),
        event(
            7,
            Some(3),
            "tool.result",
            &result_with("call_4_3", &[started("j3", "cron", None)]),
        ),
        event(8, Some(3), "message.assistant", &reply(8, 7, 0, false)),
        event(9, Some(3), "turn.ended", r#"{"reason":"completed"}"#),
    ]
}

/// 底子追加了前 `n` 条以后的账本。
fn jobs_after(n: usize) -> Ledger {
    let mut ledger = Ledger::default();
    for event in &with_jobs()[..n] {
        ledger.append(event).unwrap();
    }
    ledger
}

/// 在 `ledger` 上接着开一轮：第 `seq` 条是人说的话，下一条开回合，再下一条是一次调了一个工具的回复。交回那个调用。
fn new_turn(ledger: &mut Ledger, seq: u64) -> String {
    ledger
        .append(&event(seq, None, "message.user", SAID))
        .unwrap();
    let turn = seq + 1;
    ledger
        .append(&event(
            turn,
            Some(turn),
            "turn.started",
            &format!(r#"{{"trigger":{seq}}}"#),
        ))
        .unwrap();
    ledger
        .append(&event(
            turn + 1,
            Some(turn),
            "message.assistant",
            &reply(turn + 1, turn, 1, false),
        ))
        .unwrap();
    format!("call_{}_1", turn + 1)
}

#[test]
fn a_session_with_jobs_appends() {
    let mut ledger = jobs_after(9);
    for event in [
        event(10, None, "job.reported", &job_reported("j1", "exited")),
        event_by(
            11,
            None,
            "child.reported",
            &session(A),
            &child_reported("j2", A, "done"),
        ),
        // 一个子代理可以报好几次：留言叫醒它，它会再报；崩了以后也还能再报。
        event_by(
            12,
            None,
            "child.reported",
            &session(A),
            &child_reported("j2", A, "aborted"),
        ),
        event_by(
            13,
            None,
            "child.reported",
            &session(A),
            &child_reported("j2", A, "done"),
        ),
        event_by(
            14,
            None,
            "child.reported",
            &session(A),
            &child_reported("j2", A, "stopped"),
        ),
    ] {
        ledger.append(&event).unwrap();
    }
    assert_eq!(ledger.next_seq().get(), 15);
}

#[test]
fn job_ids_are_never_reused() {
    let mut ledger = jobs_after(9);
    let call = new_turn(&mut ledger, 10);
    refused(
        &mut ledger,
        &event(
            13,
            Some(11),
            "tool.result",
            &result_with(&call, &[started("j1", "command", None)]),
        ),
        "job j1 is already taken: job ids are never reused, even after an undo",
    );
    refused(
        &mut ledger,
        &event(
            13,
            Some(11),
            "tool.result",
            &result_with(&call, &[started("j3", "agent", Some(B))]),
        ),
        "job j3 is already taken",
    );
    // 同一条结果里也不重复。
    let twice = [
        started("j4", "command", None),
        started("j4", "command", None),
    ];
    refused(
        &mut ledger,
        &event(13, Some(11), "tool.result", &result_with(&call, &twice)),
        "job j4 is already taken",
    );
    let two = [
        started("j4", "command", None),
        started("j5", "agent", Some(B)),
    ];
    ledger
        .append(&event(
            13,
            Some(11),
            "tool.result",
            &result_with(&call, &two),
        ))
        .unwrap();
}

/// 撤掉的回合里派的也算：编号不回收，那几个任务也照样能报它们停了。
#[test]
fn job_ids_of_undone_turns_stay_taken() {
    let mut ledger = jobs_after(9);
    ledger
        .append(&event(10, None, "turn.reverted", r#"{"turns":[3]}"#))
        .unwrap();
    ledger
        .append(&event(
            11,
            None,
            "job.reported",
            &job_reported("j1", "undone"),
        ))
        .unwrap();
    ledger
        .append(&event_by(
            12,
            None,
            "child.reported",
            &session(A),
            &child_reported("j2", A, "undone"),
        ))
        .unwrap();
    let call = new_turn(&mut ledger, 13);
    refused(
        &mut ledger,
        &event(
            16,
            Some(14),
            "tool.result",
            &result_with(&call, &[started("j2", "agent", Some(B))]),
        ),
        "job j2 is already taken",
    );
    ledger
        .append(&event(
            16,
            Some(14),
            "tool.result",
            &result_with(&call, &[started("j4", "agent", Some(B))]),
        ))
        .unwrap();
}

#[test]
fn an_agent_needs_its_session_and_a_command_has_none() {
    let mut ledger = jobs_after(4);
    refused(
        &mut ledger,
        &event(
            5,
            Some(3),
            "tool.result",
            &result_with("call_4_1", &[started("j1", "agent", None)]),
        ),
        "job j1 is an agent and needs session",
    );
    refused(
        &mut ledger,
        &event(
            5,
            Some(3),
            "tool.result",
            &result_with("call_4_1", &[started("j1", "command", Some(A))]),
        ),
        "job j1 is a command and has no session",
    );
    // 后面那一个坏了，整条都不收。
    let second_bad = [started("j1", "command", None), started("j2", "agent", None)];
    refused(
        &mut ledger,
        &event(
            5,
            Some(3),
            "tool.result",
            &result_with("call_4_1", &second_bad),
        ),
        "job j2 is an agent and needs session",
    );
    // 不认识的种类不查会话。
    ledger
        .append(&event(
            5,
            Some(3),
            "tool.result",
            &result_with("call_4_1", &[started("j1", "cron", Some(A))]),
        ))
        .unwrap();
}

/// 子会话的第一条带着父会话和第几层，两格同有同无，至少第 1 层。
#[test]
fn a_child_session_has_both_parent_and_depth() {
    let body = |extra: &str| CREATED.replace("}}", &format!("}}{extra}}}"));
    for depth in [1, 2] {
        Ledger::default()
            .append(&event(
                1,
                None,
                "session.created",
                &body(&format!(r#","parent":"{PARENT}","depth":{depth}"#)),
            ))
            .unwrap();
    }
    for (extra, why) in [
        (
            format!(r#","parent":"{PARENT}","depth":0"#),
            "depth should be at least 1",
        ),
        (r#","depth":0"#.to_string(), "depth should be at least 1"),
        (
            format!(r#","parent":"{PARENT}""#),
            "parent and depth go together: a child session has both, the main session neither",
        ),
        (r#","depth":1"#.to_string(), "parent and depth go together"),
    ] {
        refused(
            &mut Ledger::default(),
            &event(1, None, "session.created", &body(&extra)),
            why,
        );
    }
}

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

/// 效果 `job.messaged`（施工 7-7）。
fn messaged(job: &str) -> String {
    format!(r#"{{"kind":"job.messaged","job":"{job}"}}"#)
}

/// 给子代理留了言（施工 7-7）：它欠一份回报，报了就不欠；留言只能给这个会话派的子代理，后台命令、不认识的种类、没派过的
/// 都拦下。在会话 A 里跑的是 `j2`；派出去过的子代理照编号列出来，带着停没停。
#[test]
fn a_message_to_a_subagent_makes_it_owe_a_report() {
    let waiting = |ledger: &Ledger| ledger.waiting_children().cloned().collect::<Vec<_>>();
    let a = SessionId::parse(A).unwrap();
    let mut ledger = jobs_after(9);
    assert_eq!(waiting(&ledger), std::slice::from_ref(&a), "还没报过");
    assert_eq!(ledger.subagent_in(&a), Some(JobId::new(2).unwrap()));
    assert_eq!(ledger.subagent_in(&SessionId::parse(B).unwrap()), None);
    let reported = event_by(
        10,
        None,
        "child.reported",
        &session(A),
        &child_reported("j2", A, "done"),
    );
    ledger.append(&reported).unwrap();
    assert!(waiting(&ledger).is_empty(), "报过了");
    let call = new_turn(&mut ledger, 11);
    for job in ["j1", "j3", "j9"] {
        let bad = event(
            14,
            Some(12),
            "tool.result",
            &result_with(&call, &[messaged(job)]),
        );
        let error = ledger.append(&bad).unwrap_err();
        assert!(
            error
                .why
                .contains(&format!("job {job} was messaged but is not a subagent")),
            "{error}"
        );
    }
    let told = event(
        14,
        Some(12),
        "tool.result",
        &result_with(&call, &[messaged("j2")]),
    );
    ledger.append(&told).unwrap();
    assert_eq!(
        waiting(&ledger),
        std::slice::from_ref(&a),
        "留了言：又欠一份"
    );
    let again = event_by(
        15,
        None,
        "child.reported",
        &session(A),
        &child_reported("j2", A, "stopped"),
    );
    ledger.append(&again).unwrap();
    assert!(waiting(&ledger).is_empty(), "报了就不欠");
    let listed: Vec<(JobId, SessionId, bool)> = ledger
        .subagents()
        .map(|(job, session, stopped)| (job, session.clone(), stopped))
        .collect();
    assert_eq!(listed, [(JobId::new(2).unwrap(), a, true)]);
}

/// 留言那次调用发出以后、结果记下以前到的回报（施工 7-7）：算回了这句留言，不再等它，不会一直等一份不再来的回报。
#[test]
fn a_report_that_arrives_while_the_message_is_on_its_way_answers_it() {
    let mut ledger = jobs_after(9);
    let first = event_by(
        10,
        None,
        "child.reported",
        &session(A),
        &child_reported("j2", A, "done"),
    );
    ledger.append(&first).unwrap();
    let call = new_turn(&mut ledger, 11);
    let quick = event_by(
        14,
        None,
        "child.reported",
        &session(A),
        &child_reported("j2", A, "done"),
    );
    ledger.append(&quick).unwrap();
    let told = event(
        15,
        Some(12),
        "tool.result",
        &result_with(&call, &[messaged("j2")]),
    );
    ledger.append(&told).unwrap();
    assert_eq!(ledger.waiting_children().count(), 0);
}
