//! 账本的测试：一段合规的会话从头追加到尾；02 第九节表里的每一条规矩各有被拦下的例子，
//! 被拦下时报错说清是哪一条，账本不变。

use super::*;

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
/// 中间一条不认识的种类；然后压缩一次，再开一轮。
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
        event(
            17,
            None,
            "context.compacted",
            r#"{"upto":16,"summary":"…"}"#,
        ),
        event(18, None, "message.user", SAID),
        event(19, Some(19), "turn.started", r#"{"trigger":18}"#),
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
        "序号应该是 1",
    );
    let mut ledger = after(2);
    refused(
        &mut ledger,
        &event(4, None, "message.user", SAID),
        "序号应该是 3",
    );
    refused(
        &mut ledger,
        &event(2, None, "message.user", SAID),
        "序号应该是 3",
    );
}

#[test]
fn only_the_first_event_is_session_created() {
    refused(
        &mut Ledger::default(),
        &event(1, None, "message.user", SAID),
        "第 1 条应该是会话创建",
    );
    let mut ledger = after(1);
    refused(
        &mut ledger,
        &event(2, None, "session.created", CREATED),
        "会话创建只能是第 1 条",
    );
}

#[test]
fn a_turn_starts_with_its_own_seq_after_its_trigger_and_alone() {
    let mut ledger = after(2);
    refused(
        &mut ledger,
        &event(3, Some(2), "turn.started", r#"{"trigger":2}"#),
        "它自己的序号",
    );
    refused(
        &mut ledger,
        &event(3, Some(3), "turn.started", r#"{"trigger":3}"#),
        "trigger 应该是回合开始之前的一条",
    );
    let mut ledger = after(3);
    ledger
        .append(&event(4, None, "message.user", SAID))
        .unwrap();
    refused(
        &mut ledger,
        &event(5, Some(5), "turn.started", r#"{"trigger":4}"#),
        "回合 3 还没有结束",
    );
}

#[test]
fn turn_must_be_the_one_in_progress() {
    let mut ledger = after(3);
    refused(
        &mut ledger,
        &event(4, Some(2), "message.user", SAID),
        "回合 2 不是正在进行的回合",
    );
    refused(
        &mut ledger,
        &event(4, None, "message.assistant", &reply(4, 3, 0, false)),
        "message.assistant 只在回合里发生",
    );
    // 回合结束以后，谁也不能再说自己属于它，不认识的种类也一样。
    let mut ledger = after(9);
    refused(
        &mut ledger,
        &event(10, Some(3), "ext.memory.recalled", r#"{"hits":[]}"#),
        "回合 3 不是正在进行的回合",
    );
}

#[test]
fn tool_calls_are_numbered_after_their_message() {
    let mut ledger = after(4);
    let wrong_order = r#"{"blocks":[{"type":"tool_call","call_id":"call_5_2","name":"read","args":"{}"}],"seen":4}"#;
    refused(
        &mut ledger,
        &event(5, Some(3), "message.assistant", wrong_order),
        "编号应该是 call_5_1",
    );
    refused(
        &mut ledger,
        &event(5, Some(3), "message.assistant", &reply(4, 4, 1, false)),
        "写的是 call_4_1",
    );
}

#[test]
fn a_result_needs_a_call_still_waiting_for_one() {
    let mut ledger = after(5);
    refused(
        &mut ledger,
        &event(6, Some(3), "tool.result", &result("call_9_1", "ok")),
        "call_9_1 不是一个还在等结果的调用",
    );
    let mut ledger = after(6);
    refused(
        &mut ledger,
        &event(7, Some(3), "tool.result", &result("call_5_2", "ok")),
        "call_5_2 不是一个还在等结果的调用",
    );
}

#[test]
fn a_turn_ends_only_when_every_call_has_a_result() {
    let mut ledger = after(6);
    refused(
        &mut ledger,
        &event(7, Some(3), "turn.ended", r#"{"reason":"completed"}"#),
        "调用 call_5_1 还没有结果",
    );
}

#[test]
fn compaction_only_moves_forward() {
    let mut ledger = after(16);
    refused(
        &mut ledger,
        &event(17, None, "context.compacted", r#"{"upto":17,"summary":""}"#),
        "应该在这一条之前",
    );
    let mut ledger = after(17);
    refused(
        &mut ledger,
        &event(18, None, "context.compacted", r#"{"upto":15,"summary":""}"#),
        "早于上一次压缩的 16",
    );
}

#[test]
fn revert_only_turns_after_the_latest_compaction() {
    let mut ledger = after(14);
    refused(
        &mut ledger,
        &event(15, None, "turn.reverted", r#"{"turns":[11,12]}"#),
        "回合 12 不存在",
    );
    // 压缩替代到 16，回合 3 和 11 都在它之前，写进了摘要，撤不了了。
    let mut ledger = after(17);
    refused(
        &mut ledger,
        &event(18, None, "turn.reverted", r#"{"turns":[3]}"#),
        "在最近一次压缩之前",
    );
}

/// 回复看到的在它自己之前，而且不早于上一条回复：后一次请求一定看过前一条回复。
#[test]
fn a_reply_saw_what_came_before_it_including_the_last_reply() {
    let mut ledger = after(4);
    refused(
        &mut ledger,
        &event(5, Some(3), "message.assistant", &reply(5, 5, 0, false)),
        "seen 5 应该在这条回复之前",
    );
    let mut ledger = after(7);
    refused(
        &mut ledger,
        &event(8, Some(3), "message.assistant", &reply(8, 4, 0, false)),
        "seen 4 早于上一条回复 5",
    );
}

/// 模型调用的记录，看到的在它自己之前（03 第三节「模型调用怎么写」）。
#[test]
fn a_model_call_saw_what_came_before_it() {
    let called = |seen: u64| format!(r#"{{"seen":{seen},"messages":1,"result":"ok"}}"#);
    let mut ledger = after(5);
    refused(
        &mut ledger,
        &event(6, Some(3), "model.called", &called(6)),
        "seen 6 应该在这一条之前",
    );
    ledger
        .append(&event(6, Some(3), "model.called", &called(4)))
        .unwrap();
}
