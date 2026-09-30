//! 派出去过的任务（施工 7-2，`docs/blueprint/kernel/history.md`「派出去过的任务」）：工具结果的效果里派的记下标题和种类；
//! 压缩换掉了派它的那一条也在；撤掉派它的那一轮标成撤掉了，恢复了去掉；只记任务的那一条不留事件；换一份有效历史时照原来
//! 的留着。回报不带回合编号，撤销时留着；由上一轮里到的回报接着开的一轮，撤它带走上一轮排着的人的话。

use super::*;
use crate::event::JobKind;
use crate::id::JobId;

/// 第一轮（3 号，由 2 号那句开）调一次工具，结果（5 号）派出去后台命令 `j1`、子代理 `j2`；6 号回一句，7 号结束。
fn dispatching() -> Vec<Event> {
    let effects = r#"[{"kind":"job.started","job":"j1","what":"command","title":"跑测试"},{"kind":"job.started","job":"j2","what":"agent","title":"查 CI","session":"01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91"}]"#;
    let body = format!(r#"{{"call_id":"call_4_1","status":"ok","blocks":[],"effects":{effects}}}"#);
    vec![
        created(),
        message(2, ALICE),
        event(3, Some(3), KERNEL, "turn.started", r#"{"trigger":2}"#),
        event(4, Some(3), MODEL, "message.assistant", &calls(4, 3, 1)),
        event(5, Some(3), KERNEL, "tool.result", &body),
        event(6, Some(3), MODEL, "message.assistant", &calls(6, 5, 0)),
        event(
            7,
            Some(3),
            KERNEL,
            "turn.ended",
            r#"{"reason":"completed"}"#,
        ),
    ]
}

/// 后台命令 `j1` 结束了，不带回合编号。
fn command_ended(seq: u64) -> Event {
    event(
        seq,
        None,
        KERNEL,
        "job.reported",
        r#"{"job":"j1","reason":"exited","exit_code":0}"#,
    )
}

fn job(n: u64) -> JobId {
    JobId::new(n).unwrap()
}

#[test]
fn the_title_and_kind_are_kept_even_after_the_start_is_compacted() {
    let mut events = dispatching();
    events.push(message(8, ALICE));
    events.extend(compaction_turn(9, 8, 7));
    let history = feed(events);
    assert!(
        !history.events().iter().any(|event| event.seq.get() == 5),
        "派它的那一条压缩掉了"
    );
    let command = history.dispatched(&job(1)).expect("压缩不丢");
    assert_eq!(
        (&command.what, command.title.as_str(), command.undone),
        (&JobKind::Command, "跑测试", false)
    );
    assert_eq!(history.dispatched(&job(2)).unwrap().what, JobKind::Agent);
    assert!(history.dispatched(&job(3)).is_none());
}

#[test]
fn undoing_the_turn_marks_its_jobs_and_redoing_clears_the_mark() {
    let mut events = dispatching();
    events.push(reverted(8, &[3]));
    let mut ledger = Ledger::default();
    let mut history = History::default();
    for event in events {
        ledger.append(&event).unwrap();
        history.append(event);
    }
    assert!(history.dispatched(&job(1)).unwrap().undone);
    let back = event(9, None, ALICE, "turn.unreverted", r#"{"turns":[3]}"#);
    history.append(back);
    assert!(!history.dispatched(&job(1)).unwrap().undone);
}

#[test]
fn an_undo_that_can_no_longer_be_restored_keeps_the_mark() {
    let mut events = dispatching();
    events.push(reverted(8, &[3]));
    events.push(message(9, ALICE));
    events.extend(turn(10, 9));
    let history = feed(events);
    assert!(
        history.dispatched(&job(2)).unwrap().undone,
        "下一轮开了也还是撤掉了"
    );
}

#[test]
fn noting_keeps_only_the_jobs_and_a_new_history_can_take_them_over() {
    let mut noted = History::whole();
    for event in dispatching() {
        noted.note(&event);
    }
    assert!(noted.events().is_empty(), "只记任务，不留事件");
    assert_eq!(noted.dispatched(&job(1)).unwrap().title, "跑测试");
    let mut rebuilt = History::whole();
    rebuilt.jobs_from(&noted);
    assert_eq!(rebuilt.dispatched(&job(2)).unwrap().title, "查 CI");
    assert_eq!(
        rebuilt.until(Seq::FIRST).dispatched(&job(1)),
        noted.dispatched(&job(1)),
        "截出来的前一段带着"
    );
    assert_eq!(
        rebuilt.after(Seq::FIRST).dispatched(&job(2)),
        noted.dispatched(&job(2)),
        "截出来的后一段带着"
    );
}

/// 3 号那一轮最后一次请求（看到 5 为止）在路上时，人又说了一句（6，排着），后台命令结束了（7，不带回合）；这一轮结束（9），
/// 由 7 接着开下一轮（10）。撤掉 10：那一轮才听到的 6 跟着撤，回报 7 是别处来的，留着。
#[test]
fn undoing_a_turn_a_report_carried_on_takes_the_message_queued_before_it() {
    let mut events = dispatching();
    events.truncate(5);
    events.push(event(6, Some(3), ALICE, "message.user", r#"{"blocks":[]}"#));
    events.push(command_ended(7));
    events.push(event(
        8,
        Some(3),
        MODEL,
        "message.assistant",
        &calls(8, 5, 0),
    ));
    events.push(event(
        9,
        Some(3),
        KERNEL,
        "turn.ended",
        r#"{"reason":"completed"}"#,
    ));
    events.extend(turn(10, 7));
    events.push(reverted(13, &[10]));
    let history = feed(events);
    assert_eq!(seqs(&history), vec![1, 2, 3, 4, 5, 7, 8, 9]);
}

/// 闲着时由回报开的一轮（回报 8，回合 9）：撤它只拿走那一轮，回报留着；上一轮崩了留下的排着的话（6）归上一轮。
#[test]
fn undoing_a_turn_a_report_opened_while_idle_takes_nothing_else() {
    let mut events = dispatching();
    events.truncate(5);
    events.push(event(6, Some(3), ALICE, "message.user", r#"{"blocks":[]}"#));
    events.push(event(
        7,
        Some(3),
        KERNEL,
        "turn.ended",
        r#"{"reason":"aborted"}"#,
    ));
    events.push(command_ended(8));
    events.extend(turn(9, 8));
    events.push(reverted(12, &[9]));
    let history = feed(events);
    assert_eq!(seqs(&history), vec![1, 2, 3, 4, 5, 6, 7, 8]);
}
