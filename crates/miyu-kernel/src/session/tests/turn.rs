//! 开回合、发请求（`docs/designs/02-内核.md` 第六节「回合怎么开、请求怎么发」）：同一批追加
//! 消息和回合的开头；开头落了盘才跑挂接点；挂接点跑完、事件全落了盘才请求；模块的注入；
//! 中途来的消息；对不上的挂接点结果；环境变了。

use super::executor::injection;
use super::*;
use crate::id::ModuleId;
use crate::origin::Module;

/// 前 5 条：造会话、消息、回合开始、两块事实。
const OPENING: [(u64, &str); 5] = [
    (1, "session.created"),
    (2, "message.user"),
    (3, "turn.started"),
    (4, "context.injected"),
    (5, "context.injected"),
];

#[test]
fn a_message_to_an_idle_session_opens_a_turn_in_the_same_batch() {
    let mut session = session();
    let actions = session.handle(send(1, "看看 src 目录"));
    let [Action::Append(events)] = actions.as_slice() else {
        panic!("消息和回合的开头应该同一批追加，别的等落了盘再说：{actions:?}");
    };
    assert_eq!(appended(&actions), seqs(&[2, 3, 4, 5]));
    for event in events {
        assert_eq!(event.at, at(1), "一条输入产生的事件，时刻相同");
        assert_eq!(event.cause, Some(id(1)), "cause 一路追得下去");
    }
    assert_eq!(events[0].turn, None, "开回合之前来的消息不属于这个回合");
    for event in &events[1..] {
        assert_eq!(event.turn, Some(turn3()));
        assert_eq!(event.by, By::Kernel);
    }
    assert!(matches!(&events[1].body, Body::TurnStarted(started) if started.trigger == seq(2)));
    let env = fact_of(&events[2]);
    assert_eq!(env.kind.as_str(), "env");
    assert_eq!(
        env.text,
        r#"<e t="Fri 2026-09-25 16:00" z="UTC+09:00" d="~/src/miyu"/>"#
    );
    let permission = fact_of(&events[3]);
    assert_eq!(permission.kind.as_str(), "permission");
    assert_eq!(permission.text, r#"<p l="workspace"/>"#);
}

#[test]
fn the_hooks_run_once_the_opening_is_stored() {
    let mut session = session();
    session.handle(send(1, "hi"));
    assert!(
        hooks(&session.handle(stored(4))).is_empty(),
        "开头还没全落盘"
    );
    let actions = session.handle(stored(5));
    assert!(matches!(actions.first(), Some(Action::Push(_))), "先推送");
    assert_eq!(hooks(&actions), [turn3()]);
    assert!(calls(&actions).is_empty(), "挂接点还没跑完");
}

#[test]
fn the_model_is_called_once_the_hooks_are_done() {
    let mut session = session();
    session.handle(send(1, "hi"));
    session.handle(stored(5));
    let actions = session.handle(hooks_done(turn3(), Vec::new()));
    assert_eq!(actions.len(), 1, "交回空的，什么都不追加：{actions:?}");
    assert_eq!(calls(&actions), [(seq(5), listed(&OPENING))]);
    // 一个回合只请求一次。
    assert!(session.handle(stored(5)).is_empty());
    assert!(session.handle(hooks_done(turn3(), Vec::new())).is_empty());
}

#[test]
fn module_facts_are_appended_in_the_order_they_came_back() {
    let mut session = session();
    session.handle(send(1, "hi"));
    session.handle(stored(5));
    let injected = vec![
        injection("memory", "<memory>likes tea</memory>"),
        injection("runtime", "<runtime/>"),
    ];
    let actions = session.handle(hooks_done(turn3(), injected));
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[6, 7]));
    for (event, module) in events.iter().zip(["memory", "runtime"]) {
        let by = By::Module(Module {
            id: ModuleId::parse(module).unwrap(),
        });
        assert_eq!(event.by, by);
        assert_eq!(event.at, at(30));
        assert_eq!(event.turn, Some(turn3()));
        assert_eq!(event.cause, Some(id(1)));
        assert_eq!(fact_of(event).kind.as_str(), module);
    }
    assert!(calls(&actions).is_empty(), "注入还没落盘");
    let mut expected = OPENING.to_vec();
    expected.extend([(6, "context.injected"), (7, "context.injected")]);
    assert_eq!(
        calls(&session.handle(stored(7))),
        [(seq(7), listed(&expected))]
    );
}

#[test]
fn a_message_during_the_turn_joins_it_without_opening_another() {
    let mut session = session();
    session.handle(send(1, "one"));
    let actions = session.handle(send(2, "two"));
    let [Action::Append(events)] = actions.as_slice() else {
        panic!("中途来的消息只追加：{actions:?}");
    };
    let [message] = events.as_slice() else {
        panic!("中途来的消息不开新回合：{events:?}");
    };
    assert_eq!(message.seq, seq(6));
    assert_eq!(message.turn, Some(turn3()));
    assert_eq!(message.cause, Some(id(2)));
    assert_eq!(message.by, alice());
    let actions = session.handle(stored(6));
    assert_eq!(hooks(&actions), [turn3()], "挂接点只跑一次");
    assert_eq!(replies(&actions).len(), 2);
    // 赶在第一次请求之前的，进这次请求。
    let mut expected = OPENING.to_vec();
    expected.push((6, "message.user"));
    assert_eq!(
        calls(&session.handle(hooks_done(turn3(), Vec::new()))),
        [(seq(6), listed(&expected))]
    );
}

#[test]
fn the_call_waits_for_a_message_that_came_while_the_hooks_ran() {
    let mut session = session();
    session.handle(send(1, "one"));
    session.handle(stored(5));
    session.handle(send(2, "two"));
    assert!(
        calls(&session.handle(hooks_done(turn3(), Vec::new()))).is_empty(),
        "6 号还没落盘"
    );
    let mut expected = OPENING.to_vec();
    expected.push((6, "message.user"));
    assert_eq!(
        calls(&session.handle(stored(6))),
        [(seq(6), listed(&expected))]
    );
}

#[test]
fn hook_results_that_do_not_match_are_ignored() {
    let mut session = session();
    let late = || vec![injection("memory", "<memory/>")];
    assert!(
        session.handle(hooks_done(turn3(), late())).is_empty(),
        "空闲时"
    );
    session.handle(send(1, "hi"));
    assert!(
        session.handle(hooks_done(turn3(), late())).is_empty(),
        "还没叫跑挂接点"
    );
    session.handle(stored(5));
    assert!(
        session
            .handle(hooks_done(TurnId::new(seq(9)), late()))
            .is_empty(),
        "别的回合的"
    );
    assert_eq!(
        calls(&session.handle(hooks_done(turn3(), Vec::new()))).len(),
        1
    );
    assert!(
        session.handle(hooks_done(turn3(), late())).is_empty(),
        "第二次来的"
    );
}

#[test]
fn the_environment_reported_last_is_the_one_injected() {
    let mut session = session();
    let moved = Environment {
        offset: UtcOffset::from_minutes(0).unwrap(),
        cwd: "~/src/other".to_string(),
    };
    assert!(
        session.handle(Input::Environment(moved)).is_empty(),
        "不当场注入"
    );
    let events = appended_events(&session.handle(send(1, "hi")));
    assert_eq!(
        fact_of(&events[2]).text,
        r#"<e t="Fri 2026-09-25 07:00" z="UTC+00:00" d="~/src/other"/>"#
    );
}
