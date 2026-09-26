//! 撤销与恢复（03 第七节「有效历史」）：撤掉那几轮，连同它们接过去的、人亲口说的话；别处来的
//! 留着；上一轮排着、被这一轮接过去的几句一起撤，上一轮听到过的留着；崩了留下的排着的，归那一轮；
//! 恢复放回原处；下一轮开始、压缩了，放在一边的就丢掉。

use super::*;

fn unreverted(seq: u64, turns: &[u64]) -> Event {
    let body = format!(r#"{{"turns":{turns:?}}}"#);
    event(seq, None, ALICE, "turn.unreverted", &body)
}

/// 回合 `turn` 里 `by` 说的一句，排着队。
fn queued(seq: u64, turn: u64, by: &str) -> Event {
    event(seq, Some(turn), by, "message.user", r#"{"blocks":[]}"#)
}

fn ended(seq: u64, turn: u64, reason: &str) -> Event {
    let body = format!(r#"{{"reason":"{reason}"}}"#);
    event(seq, Some(turn), KERNEL, "turn.ended", &body)
}

fn started(seq: u64, trigger: u64) -> Event {
    let body = format!(r#"{{"trigger":{trigger}}}"#);
    event(seq, Some(seq), KERNEL, "turn.started", &body)
}

/// 回合 `turn` 里的一条回复，没有调用，它的请求看到了第 `seen` 条为止。
fn said(seq: u64, turn: u64, seen: u64) -> Event {
    event(
        seq,
        Some(turn),
        MODEL,
        "message.assistant",
        &calls(seq, seen, 0),
    )
}

/// 撤掉第二轮：7 到 9 去掉，触发它的 6 是人亲口发的，也去掉；撤销这一条本身不留。
#[test]
fn undo_takes_the_turn_and_the_message_that_asked_for_it() {
    let mut events = two_turns();
    events.push(reverted(10, &[7]));
    let history = feed(events);
    assert_eq!(seqs(&history), vec![1, 2, 3, 4, 5]);
}

/// 从第一轮撤：第二轮跟着撤，两句触发的话都去掉。
#[test]
fn undo_from_a_turn_takes_the_turns_after_it() {
    let mut events = two_turns();
    events.push(reverted(10, &[3, 7]));
    let history = feed(events);
    assert_eq!(seqs(&history), vec![1]);
}

/// 群里别人说的话、另一个会话发来的消息、定时触发，都是别处来的：
/// 撤掉的只是她对它们的反应，它们自己留着。
#[test]
fn undo_keeps_triggers_that_came_from_elsewhere() {
    let mut events = vec![created(), message(2, GROUP_MEMBER)];
    events.extend(turn(3, 2));
    events.push(message(6, ANOTHER_SESSION));
    events.extend(turn(7, 6));
    events.push(event(10, None, TIMER, "ext.timer.fired", "{}"));
    events.extend(turn(11, 10));
    events.push(reverted(14, &[3, 7, 11]));
    let history = feed(events);
    assert_eq!(seqs(&history), vec![1, 2, 6, 10]);
}

/// 第一轮最后一次请求看到 4，这时人说了两句（5、7），群里有人插了一句（6）；回复（8）说完，
/// 第一轮结束（9），由排着的最后一句（7）接着开第二轮（10）。撤掉第二轮：5 和 7 是它接过去的，
/// 一起撤；6 是别处来的，留着。
#[test]
fn undo_takes_the_queued_messages_the_turn_picked_up() {
    let events = vec![
        created(),
        message(2, ALICE),
        started(3, 2),
        event(
            4,
            Some(3),
            KERNEL,
            "context.injected",
            r#"{"kind":"env","text":"<env/>"}"#,
        ),
        queued(5, 3, ALICE),
        queued(6, 3, GROUP_MEMBER),
        queued(7, 3, ALICE),
        said(8, 3, 4),
        ended(9, 3, "completed"),
        started(10, 7),
        said(11, 10, 10),
        ended(12, 10, "completed"),
        reverted(13, &[10]),
    ];
    assert_eq!(seqs(&feed(events)), vec![1, 2, 3, 4, 6, 8, 9]);
}

/// 第一轮调了一个工具，结果（5）回来以后人说了一句（6），下一次请求正好看到它为止（回复 7）；
/// 最后一次请求在路上时又说了一句（8），由它接着开第二轮。撤掉第二轮：只撤 8，6 是第一轮听到过的，
/// 留着。
#[test]
fn a_message_the_turn_before_heard_stays() {
    let events = vec![
        created(),
        message(2, ALICE),
        started(3, 2),
        event(4, Some(3), MODEL, "message.assistant", &calls(4, 3, 1)),
        result(5, 3, "call_4_1"),
        queued(6, 3, ALICE),
        said(7, 3, 6),
        queued(8, 3, ALICE),
        ended(9, 3, "completed"),
        started(10, 8),
        said(11, 10, 10),
        ended(12, 10, "completed"),
        reverted(13, &[10]),
    ];
    assert_eq!(seqs(&feed(events)), vec![1, 2, 3, 4, 5, 6, 7, 9]);
}

/// 请求出错了，没有回复，听到哪里看它的 `model.called`：挂接点在跑时说的一句（4）请求听到了；
/// 请求在路上时说的一句（5）没听到，出错以后由它接着开第二轮。撤掉第二轮：只撤 5。
#[test]
fn a_request_that_failed_still_heard_what_it_saw() {
    let events = vec![
        created(),
        message(2, ALICE),
        started(3, 2),
        queued(4, 3, ALICE),
        queued(5, 3, ALICE),
        event(
            6,
            Some(3),
            KERNEL,
            "model.called",
            r#"{"seen":4,"messages":1,"result":"error"}"#,
        ),
        ended(7, 3, "error"),
        started(8, 5),
        said(9, 8, 8),
        ended(10, 8, "completed"),
        reverted(11, &[8]),
    ];
    assert_eq!(seqs(&feed(events)), vec![1, 2, 3, 4, 6, 7]);
}

/// 第一轮请求在路上时人说了一句（4），核心崩了，载入时这一轮以 aborted 结束（5）。4 排在那条结束
/// 前面，归第一轮。人再开口（6）开了第二轮，撤掉第二轮：只撤 6。
#[test]
fn a_crash_leaves_its_queued_messages_with_its_turn() {
    let events = vec![
        created(),
        message(2, ALICE),
        started(3, 2),
        queued(4, 3, ALICE),
        ended(5, 3, "aborted"),
        message(6, ALICE),
        started(7, 6),
        said(8, 7, 7),
        ended(9, 7, "completed"),
        reverted(10, &[7]),
    ];
    assert_eq!(seqs(&feed(events)), vec![1, 2, 3, 4, 5]);
}

/// 恢复：撤掉的连同跟着撤的话放回原处，排出来和撤销之前一模一样；恢复这一条本身不留。
#[test]
fn redo_puts_back_what_was_undone() {
    let before = feed(two_turns());
    let mut events = two_turns();
    events.push(reverted(10, &[3, 7]));
    events.push(unreverted(11, &[3, 7]));
    let history = feed(events);
    assert_eq!(seqs(&history), seqs(&before));
    assert_eq!(ordered(&history), ordered(&before));
    assert!(history.undone.is_empty());
}

/// 两轮走完以后切了一次权限（10），再撤掉第二轮、又恢复：第二轮排回 10 前面，照日志的先后。
#[test]
fn redo_puts_them_back_where_they_were() {
    let mut events = two_turns();
    events.push(event(
        10,
        None,
        ALICE,
        "session.policy_changed",
        r#"{"permission":{"level":"workspace","read_only":true}}"#,
    ));
    events.push(reverted(11, &[7]));
    events.push(unreverted(12, &[7]));
    assert_eq!(seqs(&feed(events)), (1..=10).collect::<Vec<_>>());
}

/// 连着撤两次，一次恢复一次，从最近的往前。
#[test]
fn two_undos_come_back_one_at_a_time() {
    let mut events = two_turns();
    events.push(reverted(10, &[7]));
    events.push(reverted(11, &[3]));
    events.push(unreverted(12, &[3]));
    assert_eq!(seqs(&feed(events.clone())), vec![1, 2, 3, 4, 5]);
    events.push(unreverted(13, &[7]));
    assert_eq!(seqs(&feed(events)), (1..=9).collect::<Vec<_>>());
}

/// 下一轮开始、压缩了，放在一边的就丢掉：那以后不能再恢复。
#[test]
fn the_next_turn_or_a_compaction_drops_what_was_undone() {
    let mut events = two_turns();
    events.push(reverted(10, &[7]));
    assert_eq!(feed(events.clone()).undone.len(), 1);
    let mut next = events.clone();
    next.push(message(11, ALICE));
    next.extend(turn(12, 11));
    assert!(feed(next).undone.is_empty(), "下一轮开始，放在一边的就丢掉");
    events.push(compacted(11, 10));
    assert!(feed(events).undone.is_empty(), "压缩了也丢掉");
}
