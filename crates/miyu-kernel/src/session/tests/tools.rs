//! 调工具（`docs/designs/02-内核.md` 第六节「工具怎么调、下一步怎么走」）：一步走到底；派的先后；
//! 当场拦下的；参数修正；步数上限；执行中的输出；过时的结果；这一轮的工作目录。

use super::executor::*;
use super::*;
use crate::event::{EndReason, ToolStatus, TransientBody};
use crate::id::CallId;
use crate::origin::Tool;

fn by_tool(call_id: CallId) -> By {
    By::Tool(Tool { call_id })
}

#[test]
fn a_step_runs_its_calls_then_asks_again() {
    let mut session = asking();
    let actions = call_tools(
        &mut session,
        5,
        &[("read", r#"{"path":"a"}"#), ("read", r#"{"path":"b"}"#)],
    );
    assert_eq!(
        appended(&actions),
        seqs(&[6, 7]),
        "回复、model.called，回合接着走"
    );
    assert!(ran(&actions).is_empty(), "回复落了盘才派");
    let actions = session.handle(stored(7));
    assert_eq!(
        runs(&actions),
        [
            (
                call(6, 1),
                "read".to_string(),
                r#"{"path":"a"}"#.to_string(),
                "~/src/miyu".to_string()
            ),
            (
                call(6, 2),
                "read".to_string(),
                r#"{"path":"b"}"#.to_string(),
                "~/src/miyu".to_string()
            ),
        ],
        "两个只读的一起派"
    );
    // 结果倒着回来，照到的先后追加。
    let actions = session.handle(done(call(6, 2), "b"));
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[8]));
    assert_eq!(
        result_of(&events[0]),
        (
            call(6, 2),
            ToolStatus::Ok,
            by_tool(call(6, 2)),
            "b".to_string()
        )
    );
    assert_eq!((events[0].at, events[0].turn), (at(50), Some(turn3())));
    assert!(calls(&actions).is_empty());
    let actions = session.handle(done(call(6, 1), "a"));
    assert_eq!(appended(&actions), seqs(&[9]));
    assert!(calls(&actions).is_empty(), "结果还没落盘");
    let actions = session.handle(stored(9));
    assert_eq!(calls(&actions).first().map(|(seen, _)| *seen), Some(seq(9)));
    let events = appended_events(&answer(&mut session, 9, "看完了"));
    assert_eq!(reason_of(&events[2]), &EndReason::Completed);
}

#[test]
fn calls_that_are_not_read_only_run_alone_and_in_order() {
    let mut session = asking();
    call_tools(
        &mut session,
        5,
        &[("read", "{}"), ("write", "{}"), ("read", "{}")],
    );
    assert_eq!(ran(&session.handle(stored(7))), [call(6, 1)]);
    assert_eq!(ran(&session.handle(done(call(6, 1), "a"))), [call(6, 2)]);
    assert_eq!(ran(&session.handle(done(call(6, 2), "ok"))), [call(6, 3)]);
    let mut session = asking();
    call_tools(
        &mut session,
        5,
        &[("shell", "{}"), ("read", "{}"), ("read", "{}")],
    );
    assert_eq!(ran(&session.handle(stored(7))), [call(6, 1)]);
    assert_eq!(
        ran(&session.handle(done(call(6, 1), "ok"))),
        [call(6, 2), call(6, 3)]
    );
}

#[test]
fn unknown_tools_and_bad_arguments_are_answered_on_the_spot() {
    let mut session = asking();
    let actions = call_tools(
        &mut session,
        5,
        &[("reed", "{}"), ("read", "[1]"), ("read", r#"{"path":"a"}"#)],
    );
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[6, 7, 8, 9]));
    assert_eq!(
        result_of(&events[2]),
        (
            call(6, 1),
            ToolStatus::Error,
            By::Kernel,
            "no tool reed".to_string()
        )
    );
    assert_eq!(
        result_of(&events[3]),
        (
            call(6, 2),
            ToolStatus::Error,
            By::Kernel,
            "bad args read".to_string()
        )
    );
    assert_eq!(ran(&session.handle(stored(9))), [call(6, 3)]);
    // 全是当场拦下的：这一步当场就齐了，落了盘就请求下一次。
    let mut session = asking();
    let actions = call_tools(&mut session, 5, &[("reed", "{}")]);
    assert_eq!(appended(&actions), seqs(&[6, 7, 8]));
    let actions = session.handle(stored(8));
    assert!(ran(&actions).is_empty());
    assert_eq!(calls(&actions).first().map(|(seen, _)| *seen), Some(seq(8)));
}

#[test]
fn arguments_are_repaired_for_running_but_logged_as_given() {
    let mut session = asking();
    let given = r#"{"path":"a","limit":"5"}"#;
    let events = appended_events(&call_tools(&mut session, 5, &[("read", given)]));
    let Body::MessageAssistant(reply) = &events[0].body else {
        panic!("{events:?}");
    };
    assert!(matches!(&reply.blocks[..], [Block::ToolCall(call)] if call.args == given));
    let runs = runs(&session.handle(stored(7)));
    assert_eq!(runs[0].2, r#"{"limit":5,"path":"a"}"#);
}

#[test]
fn the_step_limit_ends_the_turn_after_its_last_step_runs() {
    let mut policy = policy();
    policy.step_limit = Some(2);
    let mut session = session_with(policy);
    session.handle(send(1, "hi"));
    session.handle(stored(5));
    session.handle(hooks_done(turn3(), Vec::new()));
    // 第一步。
    call_tools(&mut session, 5, &[("read", "{}")]);
    session.handle(stored(7));
    session.handle(done(call(6, 1), "a"));
    let actions = session.handle(stored(8));
    assert_eq!(calls(&actions).first().map(|(seen, _)| *seen), Some(seq(8)));
    // 第二步：到了上限，工具照样跑完，然后结束，不再请求。
    call_tools(&mut session, 8, &[("read", "{}")]);
    assert_eq!(ran(&session.handle(stored(10))), [call(9, 1)]);
    let actions = session.handle(done(call(9, 1), "b"));
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[11, 12]));
    assert_eq!(reason_of(&events[1]), &EndReason::StepLimit);
    let actions = session.handle(stored(12));
    assert!(calls(&actions).is_empty());
    assert!(actions.contains(&Action::RunTurnEndHooks { turn: turn3() }));
}

#[test]
fn progress_is_pushed_while_a_call_runs() {
    let mut session = asking();
    call_tools(&mut session, 5, &[("shell", "{}")]);
    let progress = |session: &mut Session, text: &str| {
        session.handle(Input::ToolProgress {
            at: at(49),
            call_id: call(6, 1),
            text: text.to_string(),
        })
    };
    assert!(progress(&mut session, "early").is_empty(), "还没派");
    session.handle(stored(7));
    let actions = progress(&mut session, "Compiling\n");
    let [Action::PushTransient(transient)] = actions.as_slice() else {
        panic!("{actions:?}");
    };
    assert_eq!(transient.by, by_tool(call(6, 1)));
    assert_eq!(
        (transient.turn, transient.cause.clone()),
        (Some(turn3()), Some(id(1)))
    );
    let TransientBody::ToolProgress(body) = &transient.body else {
        panic!("{transient:?}");
    };
    assert_eq!(
        (body.call_id, body.text.as_str()),
        (call(6, 1), "Compiling\n")
    );
    session.handle(done(call(6, 1), "ok"));
    assert!(progress(&mut session, "late").is_empty(), "已经有了结果");
}

#[test]
fn results_that_do_not_match_are_ignored() {
    let mut session = asking();
    call_tools(&mut session, 5, &[("read", "{}"), ("write", "{}")]);
    assert!(
        session.handle(done(call(6, 1), "early")).is_empty(),
        "还没派"
    );
    session.handle(stored(7));
    assert!(
        session.handle(done(call(6, 2), "not yet")).is_empty(),
        "写还在等"
    );
    assert!(
        session.handle(done(call(4, 1), "stray")).is_empty(),
        "没有这个调用"
    );
    assert_eq!(appended(&session.handle(done(call(6, 1), "a"))), seqs(&[8]));
    assert!(
        session.handle(done(call(6, 1), "again")).is_empty(),
        "已经有了结果"
    );
}

#[test]
fn a_turn_keeps_the_directory_it_started_in() {
    let mut session = asking();
    session.handle(Input::Environment(environment("~/elsewhere")));
    call_tools(&mut session, 5, &[("read", "{}")]);
    let runs = runs(&session.handle(stored(7)));
    assert_eq!(runs[0].3, "~/src/miyu");
}
