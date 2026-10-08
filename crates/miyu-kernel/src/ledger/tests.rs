//! 账本的测试：一段合规的会话从头追加到尾；02 第九节表里的每一条规矩各有被拦下的例子，
//! 被拦下时报错说清是哪一条，账本不变。撤销与恢复的在 `tests/undo.rs`，没有触发的回合在 `tests/manual.rs`，
//! 任务的几条在 `tests/jobs.rs`（施工 7-1），模型调用的记录和回顾在 `tests/model.rs`（施工 3-8 四补），跨会话在 `tests/peers.rs`（施工 C-1）、
//! `tests/said.rs`（施工 C-2）。

use super::*;

mod jobs;
mod manual;
mod model;
mod peers;
mod said;
mod triggers;
mod undo;

const CREATED: &str = r#"{"owner":"alice","venue":"local","policy":"sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","permission":{"level":"workspace","read_only":false}}"#;
const SAID: &str = r#"{"blocks":[{"type":"text","text":"看看 src 目录"}]}"#;

/// 拼一条事件：序号、所属回合、种类、`body`。外壳的其余几格用固定的写法。
fn event(seq: u64, turn: Option<u64>, kind: &str, body: &str) -> Event {
    let turn = turn.map(|t| format!(r#""turn":{t},"#)).unwrap_or_default();
    Event::from_line(&format!(
        r#"{{"seq":{seq},"at":"2026-09-25T07:00:00.000Z","kind":"{kind}",{turn}"by":{{"kind":"kernel"}},"body":{body}}}"#
    ))
    .unwrap()
}

/// 一条助手消息的 `body`：几个工具调用，编号照 `seq` 编，一个不错；它的请求看到了第 `seen` 条为止。
fn reply(seq: u64, seen: u64, calls: u32, interrupted: bool) -> String {
    let blocks: Vec<String> = (1..=calls)
        .map(|k| {
            format!(
                r#"{{"type":"tool_call","call_id":"call_{seq}_{k}","name":"read","args":"{{}}"}}"#
            )
        })
        .collect();
    let cut = if interrupted {
        r#","interrupted":true"#
    } else {
        ""
    };
    format!(r#"{{"blocks":[{}],"seen":{seen}{cut}}}"#, blocks.join(","))
}

fn result(call: &str, status: &str) -> String {
    format!(r#"{{"call_id":"{call}","status":"{status}","blocks":[]}}"#)
}

/// 一段合规的会话：第一轮调了两个工具，结果倒着回来；第二轮被打断，又被撤销；
/// 中间一条不认识的种类；然后再开一轮，一开头就压缩（压缩带着它所在的回合，施工 6-9）。
fn session() -> Vec<Event> {
    vec![
        event(1, None, "session.created", CREATED),
        event(2, None, "message.user", SAID),
        event(3, Some(3), "turn.started", r#"{"trigger":2}"#),
        event(
            4,
            Some(3),
            "context.injected",
            r#"{"kind":"env","text":"<env/>"}"#,
        ),
        event(5, Some(3), "message.assistant", &reply(5, 4, 2, false)),
        event(6, Some(3), "tool.result", &result("call_5_2", "ok")),
        event(7, Some(3), "tool.result", &result("call_5_1", "ok")),
        event(8, Some(3), "message.assistant", &reply(8, 7, 0, false)),
        event(9, Some(3), "turn.ended", r#"{"reason":"completed"}"#),
        event(10, None, "message.user", SAID),
        event(11, Some(11), "turn.started", r#"{"trigger":10}"#),
        event(12, Some(11), "message.assistant", &reply(12, 11, 1, true)),
        event(
            13,
            Some(11),
            "tool.result",
            &result("call_12_1", "cancelled"),
        ),
        event(14, Some(11), "turn.ended", r#"{"reason":"interrupted"}"#),
        event(15, None, "turn.reverted", r#"{"turns":[11]}"#),
        event(16, None, "ext.memory.recalled", r#"{"hits":[]}"#),
        event(17, None, "message.user", SAID),
        event(18, Some(18), "turn.started", r#"{"trigger":17}"#),
        event(
            19,
            Some(18),
            "context.compacted",
            r#"{"upto":16,"summary":"…"}"#,
        ),
    ]
}

/// 合规的会话追加了前 `n` 条以后的账本。
fn after(n: usize) -> Ledger {
    let mut ledger = Ledger::default();
    for event in &session()[..n] {
        ledger.append(event).unwrap();
    }
    ledger
}

/// 这一条要被拦下：报错里有 `why`、写着它的序号，账本一点没变。
fn refused(ledger: &mut Ledger, event: &Event, why: &str) {
    let before = ledger.clone();
    let err = ledger.append(event).unwrap_err();
    assert!(err.to_string().contains(why), "报错里没有「{why}」：{err}");
    // 说的是英文，写进运行日志（施工 4-9 再补四中）。
    let head = format!("event {} cannot be appended: ", event.seq);
    assert!(err.to_string().starts_with(&head), "{err}");
    assert!(err.to_string().is_ascii(), "{err}");
    assert_eq!(err.seq, event.seq);
    assert_eq!(*ledger, before, "被拦下时账本不能变");
}

#[test]
fn a_whole_session_appends() {
    let ledger = after(session().len());
    assert_eq!(ledger.next_seq().get(), 20);
}

#[test]
fn seq_starts_at_one_and_follows_on() {
    refused(
        &mut Ledger::default(),
        &event(2, None, "session.created", CREATED),
        "seq should be 1",
    );
    let mut ledger = after(2);
    refused(
        &mut ledger,
        &event(4, None, "message.user", SAID),
        "seq should be 3",
    );
    refused(
        &mut ledger,
        &event(2, None, "message.user", SAID),
        "seq should be 3",
    );
}

#[test]
fn only_the_first_event_is_session_created() {
    refused(
        &mut Ledger::default(),
        &event(1, None, "message.user", SAID),
        "the first event should be session.created",
    );
    let mut ledger = after(1);
    refused(
        &mut ledger,
        &event(2, None, "session.created", CREATED),
        "session.created can only be the first event",
    );
}

#[test]
fn a_turn_starts_with_its_own_seq_after_its_trigger_and_alone() {
    let mut ledger = after(2);
    refused(
        &mut ledger,
        &event(3, Some(2), "turn.started", r#"{"trigger":2}"#),
        "its own seq as turn",
    );
    refused(
        &mut ledger,
        &event(3, Some(3), "turn.started", r#"{"trigger":3}"#),
        "trigger should be an event before the turn started",
    );
    let mut ledger = after(3);
    ledger
        .append(&event(4, None, "message.user", SAID))
        .unwrap();
    refused(
        &mut ledger,
        &event(5, Some(5), "turn.started", r#"{"trigger":4}"#),
        "turn 3 has not ended",
    );
}

#[test]
fn turn_must_be_the_one_in_progress() {
    let mut ledger = after(3);
    refused(
        &mut ledger,
        &event(4, Some(2), "message.user", SAID),
        "turn 2 is not the running turn",
    );
    refused(
        &mut ledger,
        &event(4, None, "message.assistant", &reply(4, 3, 0, false)),
        "message.assistant happens only in a turn",
    );
    // 回合结束以后，谁也不能再说自己属于它，不认识的种类也一样。
    let mut ledger = after(9);
    refused(
        &mut ledger,
        &event(10, Some(3), "ext.memory.recalled", r#"{"hits":[]}"#),
        "turn 3 is not the running turn",
    );
}

#[test]
fn tool_calls_are_numbered_after_their_message() {
    let mut ledger = after(4);
    let wrong_order = r#"{"blocks":[{"type":"tool_call","call_id":"call_5_2","name":"read","args":"{}"}],"seen":4}"#;
    refused(
        &mut ledger,
        &event(5, Some(3), "message.assistant", wrong_order),
        "should have id call_5_1",
    );
    refused(
        &mut ledger,
        &event(5, Some(3), "message.assistant", &reply(4, 4, 1, false)),
        "got call_4_1",
    );
}

#[test]
fn a_result_needs_a_call_still_waiting_for_one() {
    let mut ledger = after(5);
    refused(
        &mut ledger,
        &event(6, Some(3), "tool.result", &result("call_9_1", "ok")),
        "call_9_1 is not a call waiting for a result",
    );
    let mut ledger = after(6);
    refused(
        &mut ledger,
        &event(7, Some(3), "tool.result", &result("call_5_2", "ok")),
        "call_5_2 is not a call waiting for a result",
    );
}

#[test]
fn a_turn_ends_only_when_every_call_has_a_result() {
    let mut ledger = after(6);
    refused(
        &mut ledger,
        &event(7, Some(3), "turn.ended", r#"{"reason":"completed"}"#),
        "call call_5_1 has no result",
    );
}

#[test]
fn compaction_only_moves_forward() {
    let mut ledger = after(18);
    refused(
        &mut ledger,
        &event(
            19,
            Some(18),
            "context.compacted",
            r#"{"upto":19,"summary":""}"#,
        ),
        "should come before this event",
    );
    let mut ledger = after(19);
    refused(
        &mut ledger,
        &event(
            20,
            Some(18),
            "context.compacted",
            r#"{"upto":15,"summary":""}"#,
        ),
        "is before the last compaction's 16",
    );
}

/// 压缩跟着它所在的回合撤（施工 6-9）：不带回合的压缩没有哪一轮撤得掉，账本不收。
#[test]
fn a_compaction_happens_only_in_a_turn() {
    let mut ledger = after(16);
    refused(
        &mut ledger,
        &event(17, None, "context.compacted", r#"{"upto":16,"summary":""}"#),
        "context.compacted happens only in a turn and needs turn",
    );
}

/// 回复看到的在它自己之前，而且不早于上一条回复：后一次请求一定看过前一条回复。
#[test]
fn a_reply_saw_what_came_before_it_including_the_last_reply() {
    let mut ledger = after(4);
    refused(
        &mut ledger,
        &event(5, Some(3), "message.assistant", &reply(5, 5, 0, false)),
        "seen 5 should come before this reply",
    );
    let mut ledger = after(7);
    refused(
        &mut ledger,
        &event(8, Some(3), "message.assistant", &reply(8, 4, 0, false)),
        "seen 4 is before the previous reply 5",
    );
}

/// 撤回的都是正在进行的回合里排着队的消息（02 第六节「排队的消息」）：撤了听到过的，
/// 发出去过的请求前缀就断。
#[test]
fn only_queued_messages_can_be_withdrawn() {
    let said = |seq: u64, turn: Option<u64>| event(seq, turn, "message.user", SAID);
    let called = |seq: u64, seen: u64| {
        event(
            seq,
            Some(3),
            "model.called",
            &format!(r#"{{"seen":{seen},"messages":1,"result":"ok"}}"#),
        )
    };
    let withdraw = |seq: u64, turn: Option<u64>, messages: &str| {
        event(
            seq,
            turn,
            "message.withdrawn",
            &format!(r#"{{"messages":{messages}}}"#),
        )
    };
    // 1 创建、2 消息、3 回合开始、4 请求看到了 3、5 排着队的消息。
    let opening = [
        event(1, None, "session.created", CREATED),
        said(2, None),
        event(3, Some(3), "turn.started", r#"{"trigger":2}"#),
        called(4, 3),
        said(5, Some(3)),
    ];
    let fresh = || {
        let mut ledger = Ledger::default();
        for event in &opening {
            ledger.append(event).unwrap();
        }
        ledger
    };
    let mut ledger = fresh();
    refused(
        &mut ledger,
        &withdraw(6, Some(3), "[2]"),
        "event 2 is not a queued message of the running turn",
    );
    refused(&mut ledger, &withdraw(6, Some(3), "[4]"), "event 4 is not");
    refused(
        &mut ledger,
        &withdraw(6, Some(3), "[]"),
        "the list of withdrawn messages is empty",
    );
    refused(
        &mut ledger,
        &withdraw(6, Some(3), "[5,5]"),
        "event 5 is not",
    );
    refused(
        &mut ledger,
        &withdraw(6, None, "[5]"),
        "message.withdrawn happens only in a turn",
    );
    ledger.append(&withdraw(6, Some(3), "[5]")).unwrap();
    refused(
        &mut ledger,
        &withdraw(7, Some(3), "[5]"),
        "already withdrawn",
    );
    // 听到过的撤不了：请求看到了第 5 条。
    let mut ledger = fresh();
    ledger.append(&called(6, 5)).unwrap();
    refused(
        &mut ledger,
        &withdraw(7, Some(3), "[5]"),
        "already seen by a request",
    );
}

#[test]
fn requests_and_decisions_follow_the_calls() {
    let requested = |seq: u64, call: &str| {
        event(
            seq,
            Some(3),
            "tool.approval_requested",
            &format!(r#"{{"call_id":"{call}","access":"write"}}"#),
        )
    };
    let decided = |seq: u64, call: &str| {
        event(
            seq,
            Some(3),
            "tool.approval_decided",
            &format!(r#"{{"call_id":"{call}","decision":"once"}}"#),
        )
    };
    // 前 5 条：第 5 条回复里有两个调用，都还没有结果。
    let mut ledger = after(5);
    refused(
        &mut ledger,
        &requested(6, "call_4_1"),
        "call_4_1 is not a call waiting for a result",
    );
    refused(
        &mut ledger,
        &decided(6, "call_5_1"),
        "call_5_1 is not waiting for approval",
    );
    refused(
        &mut ledger,
        &event(
            6,
            None,
            "tool.approval_requested",
            r#"{"call_id":"call_5_1","access":"write"}"#,
        ),
        "tool.approval_requested happens only in a turn",
    );
    ledger.append(&requested(6, "call_5_1")).unwrap();
    refused(
        &mut ledger,
        &requested(7, "call_5_1"),
        "already has a pending approval request",
    );
    ledger.append(&decided(7, "call_5_1")).unwrap();
    refused(&mut ledger, &decided(8, "call_5_1"), "already decided");
    // 结果了结请求：有了结果，就不能再决定，也不能再请求。
    ledger.append(&requested(8, "call_5_2")).unwrap();
    let skipped = event(9, Some(3), "tool.result", &result("call_5_2", "skipped"));
    ledger.append(&skipped).unwrap();
    refused(
        &mut ledger,
        &decided(10, "call_5_2"),
        "call_5_2 is not waiting for approval",
    );
    refused(
        &mut ledger,
        &requested(10, "call_5_2"),
        "call_5_2 is not a call waiting for a result",
    );
}

#[test]
fn questions_and_answers_follow_the_calls() {
    let asked = |seq: u64, call: &str| {
        event(
            seq,
            Some(3),
            "question.asked",
            &format!(r#"{{"call_id":"{call}","questions":[{{"question":"?"}}]}}"#),
        )
    };
    let answered = |seq: u64, call: &str| {
        event(
            seq,
            Some(3),
            "question.answered",
            &format!(r#"{{"call_id":"{call}","answers":[{{}}]}}"#),
        )
    };
    // 前 5 条：第 5 条回复里有两个调用，都还没有结果。
    let mut ledger = after(5);
    refused(
        &mut ledger,
        &asked(6, "call_4_1"),
        "call_4_1 is not a call waiting for a result",
    );
    refused(
        &mut ledger,
        &answered(6, "call_5_1"),
        "call_5_1 is not waiting for answers",
    );
    refused(
        &mut ledger,
        &event(
            6,
            None,
            "question.asked",
            r#"{"call_id":"call_5_1","questions":[]}"#,
        ),
        "question.asked happens only in a turn",
    );
    ledger.append(&asked(6, "call_5_1")).unwrap();
    refused(
        &mut ledger,
        &asked(7, "call_5_1"),
        "already has pending questions",
    );
    ledger.append(&answered(7, "call_5_1")).unwrap();
    refused(&mut ledger, &answered(8, "call_5_1"), "already answered");
    // 答完了还能再问；结果了结题目。
    ledger.append(&asked(8, "call_5_1")).unwrap();
    let result = event(9, Some(3), "tool.result", &result("call_5_1", "cancelled"));
    ledger.append(&result).unwrap();
    refused(
        &mut ledger,
        &answered(10, "call_5_1"),
        "call_5_1 is not waiting for answers",
    );
}
