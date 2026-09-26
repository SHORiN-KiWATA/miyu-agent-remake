//! 打断和急着插话（`docs/designs/02-内核.md` 第六节「打断和急着插话」）：空闲时拒绝；各个阶段
//! 打断；半截回复；在跑的和没派的工具；回应附上的序号；急着插话跳过没跑的。

use super::executor::*;
use super::*;
use crate::accumulate::{Delta, Kind};
use crate::event::{CallResult, EndReason, ToolStatus};
use crate::id::CallId;

/// alice 在 07:00:`second` 打断，命令编号是 `n`。
fn stop(n: u64, second: u64) -> Input {
    Input::Command(Received {
        id: id(n),
        by: alice(),
        at: at(second),
        command: Command::Interrupt,
    })
}

/// 一条工具结果：编号、状态、`by`、`cause`、内容。
fn result_of(event: &Event) -> (CallId, ToolStatus, By, Option<CommandId>, String) {
    let Body::ToolResult(result) = &event.body else {
        panic!("应该是 tool.result：{event:?}");
    };
    let [Block::Text(text)] = result.blocks.as_slice() else {
        panic!("{result:?}");
    };
    (
        result.call_id,
        result.status.clone(),
        event.by.clone(),
        event.cause.clone(),
        text.text.clone(),
    )
}

/// 回合被 alice 用命令 `n` 打断而结束。
fn interrupted_by(event: &Event, n: u64) {
    assert_eq!(reason_of(event), &EndReason::Interrupted);
    assert_eq!(
        (event.by.clone(), event.cause.clone()),
        (alice(), Some(id(n)))
    );
}

#[test]
fn interrupting_an_idle_session_is_refused() {
    let mut session = session();
    assert_eq!(
        session.handle(interrupt(1)),
        [Action::Reply {
            id: id(1),
            outcome: Outcome::Rejected {
                reason: Reason::NotRunning
            },
        }]
    );
    assert_eq!(Reason::NotRunning.code(), "not_running");
}

#[test]
fn interrupting_before_the_request_just_ends_the_turn() {
    // 开头还没落盘。
    let mut session = session();
    session.handle(send(1, "hi"));
    let actions = session.handle(stop(2, 10));
    let [Action::Append(events)] = actions.as_slice() else {
        panic!("{actions:?}");
    };
    assert_eq!(appended(&actions), seqs(&[6]));
    interrupted_by(&events[0], 2);
    assert_eq!(events[0].turn, Some(turn3()));
    let actions = session.handle(stored(6));
    assert!(actions.contains(&accepted_reply(2, &[6])));
    assert!(actions.contains(&Action::RunTurnEndHooks { turn: turn3() }));
    assert!(hooks(&actions).is_empty(), "打断了，不再跑回合开始的挂接点");
    // 挂接点在跑：之后交回来的不理。
    let mut hooking = super::session();
    hooking.handle(send(1, "hi"));
    hooking.handle(stored(5));
    interrupted_by(&appended_events(&hooking.handle(stop(2, 10)))[0], 2);
    assert!(hooking.handle(hooks_done(turn3(), Vec::new())).is_empty());
    // 会话空闲了，下一条消息开下一个回合。
    hooking.handle(stored(6));
    assert_eq!(appended(&hooking.handle(send(3, "再来"))), seqs(&[7, 8]));
}

#[test]
fn interrupting_a_request_keeps_what_came_and_cancels_its_calls() {
    let mut session = asking();
    session.handle(sent(5));
    let pieces = [
        Delta::Start {
            index: 0,
            kind: Kind::Text,
        },
        Delta::Text {
            index: 0,
            text: "我先".to_string(),
        },
        Delta::Start {
            index: 1,
            kind: Kind::ToolCall {
                name: "read".to_string(),
            },
        },
        Delta::Text {
            index: 1,
            text: "{}".to_string(),
        },
        Delta::End { index: 1 },
        Delta::Start {
            index: 2,
            kind: Kind::ToolCall {
                name: "write".to_string(),
            },
        },
        Delta::Text {
            index: 2,
            text: "{".to_string(),
        },
    ];
    for piece in pieces {
        session.handle(delta(5, 41, piece));
    }
    let actions = session.handle(stop(2, 47));
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[6, 7, 8, 9]));
    assert_eq!(actions.last(), Some(&Action::CancelModel { seen: seq(5) }));
    // 半截回复：正文留下，收全了的调用留下，没收全的丢掉。
    let Body::MessageAssistant(reply) = &events[0].body else {
        panic!("{events:?}");
    };
    assert!(reply.interrupted);
    assert_eq!(reply.seen, seq(5));
    assert_eq!(reply.blocks.len(), 2);
    assert!(matches!(&reply.blocks[1], Block::ToolCall(tool) if tool.call_id == call(6, 1)));
    assert_eq!(
        (events[0].by.clone(), events[0].cause.clone()),
        (By::Model(deepseek()), Some(id(1)))
    );
    // model.called：结果是被打断，用时算到打断为止。
    let called = called_of(&events[1]);
    assert_eq!(called.result, CallResult::Interrupted);
    assert_eq!((called.usage, called.duration_ms), (None, Some(7000)));
    assert_eq!(events[1].cause, Some(id(1)));
    // 留下的调用补「已取消，没跑过」，by 是打断的人。
    assert_eq!(
        result_of(&events[2]),
        (
            call(6, 1),
            ToolStatus::Cancelled,
            alice(),
            Some(id(2)),
            "cancelled before".to_string()
        )
    );
    interrupted_by(&events[3], 2);
    // 回应附上这一次追加的全部序号；之后到的增量、结局不理。
    assert!(
        session
            .handle(stored(9))
            .contains(&accepted_reply(2, &[6, 7, 8, 9]))
    );
    assert!(
        session
            .handle(delta(5, 48, Delta::End { index: 2 }))
            .is_empty()
    );
    assert!(session.handle(ended(5)).is_empty());
}

#[test]
fn interrupting_a_request_that_got_nothing_writes_no_reply() {
    let mut session = asking();
    let actions = session.handle(stop(2, 47));
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[6, 7]));
    let called = called_of(&events[0]);
    assert_eq!(called.result, CallResult::Interrupted);
    assert_eq!((called.endpoint.clone(), called.duration_ms), (None, None));
    interrupted_by(&events[1], 2);
    assert!(actions.contains(&Action::CancelModel { seen: seq(5) }));
}

#[test]
fn interrupting_tools_cancels_the_calls_without_results() {
    let mut session = asking();
    call_tools(
        &mut session,
        5,
        &[("read", "{}"), ("read", "{}"), ("write", "{}")],
    );
    assert_eq!(ran(&session.handle(stored(7))), [call(6, 1), call(6, 2)]);
    session.handle(done(call(6, 1), "a"));
    let actions = session.handle(stop(2, 51));
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[9, 10, 11]));
    assert_eq!(
        actions[1..],
        [Action::CancelTool {
            call_id: call(6, 2)
        }],
        "只叫停在跑的"
    );
    assert_eq!(
        result_of(&events[0]),
        (
            call(6, 2),
            ToolStatus::Cancelled,
            alice(),
            Some(id(2)),
            "cancelled running".to_string()
        )
    );
    assert_eq!(result_of(&events[1]).4, "cancelled before");
    interrupted_by(&events[2], 2);
    assert!(
        session.handle(done(call(6, 2), "late")).is_empty(),
        "之后到的结果不理"
    );
}

#[test]
fn an_urgent_message_skips_the_calls_not_yet_run() {
    let mut session = asking();
    call_tools(&mut session, 5, &[("write", "{}"), ("read", "{}")]);
    assert_eq!(ran(&session.handle(stored(7))), [call(6, 1)]);
    let actions = session.handle(urgent(2, "等等，先别往下做"));
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[8, 9]));
    assert_eq!(events[0].turn, Some(turn3()));
    assert_eq!(
        result_of(&events[1]),
        (
            call(6, 2),
            ToolStatus::Skipped,
            alice(),
            Some(id(2)),
            "skipped".to_string()
        )
    );
    // 在跑的照常跑完，这一步齐了、落了盘，请求下一次，那句话在里面。
    assert_eq!(
        appended(&session.handle(done(call(6, 1), "ok"))),
        seqs(&[10])
    );
    let calls = calls(&session.handle(stored(10)));
    assert_eq!(calls[0].0, seq(10));
    assert!(calls[0].1.contains("8 message.user"));
}

#[test]
fn an_urgent_message_during_a_request_skips_the_calls_of_its_reply() {
    let mut session = asking();
    assert_eq!(appended(&session.handle(urgent(2, "换个做法"))), seqs(&[6]));
    let actions = call_tools(&mut session, 5, &[("read", "{}"), ("reed", "{}")]);
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[7, 8, 9, 10]));
    assert_eq!(result_of(&events[2]).1, ToolStatus::Skipped);
    assert_eq!(result_of(&events[3]).1, ToolStatus::Skipped, "全都跳过");
    let actions = session.handle(stored(10));
    assert!(ran(&actions).is_empty());
    assert_eq!(calls(&actions)[0].0, seq(10));
    // 插话用过了：下一次回复里的调用照常派。
    call_tools(&mut session, 10, &[("read", "{}")]);
    assert_eq!(ran(&session.handle(stored(12))), [call(11, 1)]);
}

#[test]
fn an_urgent_message_to_an_idle_session_opens_a_turn() {
    let mut session = session();
    assert_eq!(
        appended(&session.handle(urgent(1, "hi"))),
        seqs(&[2, 3, 4, 5])
    );
}

#[test]
fn an_urgent_message_before_any_call_ran_moves_straight_on() {
    let mut session = asking();
    // 回复还没落盘，一个调用都还没派。
    call_tools(&mut session, 5, &[("read", "{}"), ("read", "{}")]);
    let actions = session.handle(urgent(2, "别读了"));
    assert_eq!(
        appended(&actions),
        seqs(&[8, 9, 10]),
        "消息和两条「已跳过」"
    );
    // 这一步当场就齐了：落了盘就请求下一次，一个调用都不派。
    let actions = session.handle(stored(10));
    assert!(ran(&actions).is_empty());
    assert_eq!(calls(&actions)[0].0, seq(10));
}
