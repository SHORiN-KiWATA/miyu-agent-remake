//! 收回复、结束回合（`docs/designs/02-内核.md` 第六节「回复怎么收、回合怎么结束」）：一轮走到底；
//! `model.called` 的每一格；第二轮只注入变了的；出错；执行器违约；过时的回报；有工具调用的回复。

use super::executor::*;
use super::*;
use crate::accumulate::{Delta, Kind};
use crate::block::{Private, Reasoning};
use crate::event::{CallError, CallResult, EndReason, ErrorClass, Part, Piece, TransientBody};
use crate::id::ContentHash;

#[test]
fn a_whole_turn_from_the_message_to_the_end() {
    let mut session = asking();
    assert!(session.handle(sent(5)).is_empty());
    let mut pushed = Vec::new();
    for piece in words(0, "你好") {
        let actions = session.handle(delta(5, 41, piece));
        let [Action::PushTransient(transient)] = actions.as_slice() else {
            panic!("每段增量推一条瞬时事件：{actions:?}");
        };
        assert_eq!(transient.at, at(41));
        assert_eq!(transient.turn, Some(turn3()));
        assert_eq!(transient.by, By::Model(deepseek()));
        assert_eq!(transient.cause, Some(id(1)));
        let TransientBody::ModelDelta(body) = &transient.body else {
            panic!("应该是 model.delta：{transient:?}");
        };
        assert_eq!((body.seen, body.index), (seq(5), 0));
        pushed.push(body.piece.clone());
    }
    assert_eq!(
        pushed,
        [
            Piece::Start(Kind::Text),
            Piece::Text("你好".to_string()),
            Piece::End
        ]
    );
    let actions = session.handle(ended(5));
    let [Action::Append(events)] = actions.as_slice() else {
        panic!("说完了只追加：{actions:?}");
    };
    assert_eq!(appended(&actions), seqs(&[6, 7, 8]));
    let Body::MessageAssistant(reply) = &events[0].body else {
        panic!("先是回复：{events:?}");
    };
    assert_eq!(reply.seen, seq(5));
    assert!(!reply.interrupted);
    assert_eq!(
        reply.blocks,
        [Block::Text(Text {
            text: "你好".to_string()
        })]
    );
    assert_eq!(events[0].by, By::Model(deepseek()));
    assert_eq!(
        (events[1].by.clone(), events[2].by.clone()),
        (By::Kernel, By::Kernel)
    );
    assert_eq!(called_of(&events[1]).result, CallResult::Ok);
    assert_eq!(reason_of(&events[2]), &EndReason::Completed);
    for event in events {
        assert_eq!(event.at, at(45));
        assert_eq!(event.turn, Some(turn3()));
        assert_eq!(event.cause, Some(id(1)));
    }
    // 回合结束的挂接点，等 turn.ended 落了盘才跑。
    assert!(
        !session
            .handle(stored(7))
            .iter()
            .any(|action| matches!(action, Action::RunTurnEndHooks { .. }))
    );
    let actions = session.handle(stored(8));
    assert_eq!(
        actions.last(),
        Some(&Action::RunTurnEndHooks { turn: turn3() })
    );
    // 会话空闲了：下一条消息开下一个回合。
    let events = appended_events(&session.handle(send(2, "再来")));
    assert!(matches!(&events[1].body, Body::TurnStarted(started) if started.trigger == seq(9)));
    assert_eq!(events[1].turn, Some(TurnId::new(seq(10))));
}

#[test]
fn model_called_records_every_part_of_the_call() {
    let mut session = asking();
    let actions = answer(&mut session, 5, "你好");
    let events = appended_events(&actions);
    let called = called_of(&events[1]);
    assert_eq!(called.seen, seq(5));
    assert_eq!(
        (called.endpoint.clone(), called.model.clone()),
        (Some(deepseek().endpoint), Some(deepseek().model))
    );
    assert_eq!(called.request, Some(ContentHash::parse(REQUEST).unwrap()));
    assert_eq!(called.messages, 5, "前 5 条，替身的组装一条一条消息");
    assert_eq!(
        called.first_difference, None,
        "会话的第一次请求，没有可比的"
    );
    assert_eq!(called.usage, Some(usage()));
    assert_eq!(
        called.first_token_ms,
        Some(1000),
        "07:00:40 发出去，41 来了第一段"
    );
    assert_eq!(called.duration_ms, Some(5000), "45 说完");
    assert_eq!(called.error, None);
}

#[test]
fn a_second_request_that_only_extends_the_first_has_no_first_difference() {
    let mut session = asking();
    answer(&mut session, 5, "你好");
    session.handle(stored(8));
    session.handle(send(2, "再来"));
    session.handle(stored(10));
    let actions = session.handle(hooks_done(TurnId::new(seq(10)), Vec::new()));
    assert_eq!(
        calls(&actions).first().map(|(seen, _)| *seen),
        Some(seq(10))
    );
    let events = appended_events(&answer(&mut session, 10, "好的"));
    let called = called_of(&events[1]);
    assert_eq!(called.messages, 10);
    assert_eq!(called.first_difference, None);
}

/// 替身的组装，system 每次都不一样：前缀从 system 那里断开。
struct Drifting;

impl Assembler for Drifting {
    fn assemble(&self, history: &History) -> Request {
        let mut request = Listing.assemble(history);
        request.system = format!("{} events", history.events().len());
        request
    }
}

#[test]
fn a_rewritten_system_is_the_first_difference() {
    let created: SessionCreated = serde_json::from_str(CREATED).unwrap();
    let mut policy = policy();
    policy.assembler = Box::new(Drifting);
    let (mut session, _) = Session::create(
        id(0),
        alice(),
        at(0),
        created,
        policy,
        environment("~/src/miyu"),
    );
    session.handle(stored(1));
    session.handle(send(1, "hi"));
    session.handle(stored(5));
    session.handle(hooks_done(turn3(), Vec::new()));
    answer(&mut session, 5, "你好");
    session.handle(stored(8));
    session.handle(send(2, "再来"));
    session.handle(stored(10));
    session.handle(hooks_done(TurnId::new(seq(10)), Vec::new()));
    let events = appended_events(&answer(&mut session, 10, "好的"));
    let difference = called_of(&events[1]).first_difference.clone().unwrap();
    assert_eq!(difference.part, Part::System);
}

#[test]
fn the_next_turns_inject_only_what_changed() {
    let mut session = asking();
    answer(&mut session, 5, "你好");
    session.handle(stored(8));
    // 同一个小时、同一个目录：不注入。
    let actions = session.handle(send(2, "再来"));
    assert_eq!(appended(&actions), seqs(&[9, 10]));
    session.handle(stored(10));
    session.handle(hooks_done(TurnId::new(seq(10)), Vec::new()));
    answer(&mut session, 10, "好的");
    session.handle(stored(13));
    // 换了目录：只注入环境。
    session.handle(Input::Environment(environment("~/src/other")));
    let events = appended_events(&session.handle(send(3, "换个地方")));
    assert_eq!(events.len(), 3);
    assert_eq!(fact_of(&events[2]).kind.as_str(), "env");
    assert!(fact_of(&events[2]).text.contains("~/src/other"));
    session.handle(stored(16));
    session.handle(hooks_done(TurnId::new(seq(15)), Vec::new()));
    answer(&mut session, 16, "好");
    session.handle(stored(19));
    // 过了整点：只注入环境，时间到了 17:00。
    let next_hour = Input::Command(Received {
        id: id(4),
        by: alice(),
        at: Timestamp::parse("2026-09-25T08:00:01.000Z").unwrap(),
        command: Command::Send {
            blocks: vec![Block::Text(Text {
                text: "一小时以后".to_string(),
            })],
        },
    });
    let events = appended_events(&session.handle(next_hour));
    assert_eq!(events.len(), 3);
    assert!(fact_of(&events[2]).text.contains("17:00"));
}

#[test]
fn an_error_ends_the_turn_without_a_reply() {
    let mut session = asking();
    session.handle(sent(5));
    session.handle(delta(5, 41, words(0, "半截").remove(0)));
    let actions = session.handle(failed(5, ErrorClass::RateLimited, "429 Too Many Requests"));
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[6, 7]), "收到的半截不写成回复");
    let called = called_of(&events[0]);
    assert_eq!(called.result, CallResult::Error);
    assert_eq!(
        called.error,
        Some(CallError {
            class: ErrorClass::RateLimited,
            message: "429 Too Many Requests".to_string(),
        })
    );
    assert_eq!(called.duration_ms, Some(5000));
    assert_eq!(reason_of(&events[1]), &EndReason::Error);
}

#[test]
fn a_failure_before_sending_has_no_endpoint_or_times() {
    let mut session = asking();
    let events = appended_events(&session.handle(failed(5, ErrorClass::Auth, "no key")));
    let called = called_of(&events[0]);
    assert_eq!(
        (called.endpoint.clone(), called.request.clone()),
        (None, None)
    );
    assert_eq!((called.first_token_ms, called.duration_ms), (None, None));
    assert_eq!(reason_of(&events[1]), &EndReason::Error);
}

/// 执行器和驱动违约：各按出错算，写明哪里错。增量出的错，叫执行器别再发了。
#[test]
fn broken_reports_are_errors() {
    let bad = |session: &mut Session, input: Input, cancel: bool, why: &str| {
        let actions = session.handle(input);
        let events = appended_events(&actions);
        let error = called_of(&events[0]).error.clone().unwrap();
        assert_eq!(error.class, ErrorClass::BadStream);
        assert!(error.message.contains(why), "{}", error.message);
        assert_eq!(reason_of(&events[1]), &EndReason::Error);
        let cancelled = actions.contains(&Action::CancelModel { seen: seq(5) });
        assert_eq!(cancelled, cancel, "{actions:?}");
    };
    let text = Delta::Text {
        index: 0,
        text: "x".to_string(),
    };
    let mut session = asking();
    bad(
        &mut session,
        delta(5, 41, text.clone()),
        true,
        "还没发出去就来了增量",
    );
    let mut session = asking();
    session.handle(sent(5));
    bad(&mut session, delta(5, 41, text), true, "这一块还没开始");
    let mut session = asking();
    bad(&mut session, ended(5), false, "还没发出去就说完了");
    let mut session = asking();
    session.handle(sent(5));
    let actions = session.handle(ended(5));
    let error = called_of(&appended_events(&actions)[0])
        .error
        .clone()
        .unwrap();
    assert_eq!(error.class, ErrorClass::EmptyReply);
}

#[test]
fn stale_reports_are_ignored() {
    let mut session = asking();
    assert!(session.handle(sent(4)).is_empty());
    assert!(
        session
            .handle(delta(4, 41, words(0, "x").remove(0)))
            .is_empty()
    );
    assert!(session.handle(ended(4)).is_empty());
    session.handle(sent(5));
    // 报两次「发出去了」，只认第一次：用时从第一次算。
    session.handle(Input::RequestSent {
        at: at(44),
        seen: seq(5),
        model: deepseek(),
        request: ContentHash::parse(REQUEST).unwrap(),
    });
    for piece in words(0, "你好") {
        session.handle(delta(5, 41, piece));
    }
    let events = appended_events(&session.handle(ended(5)));
    assert_eq!(called_of(&events[1]).duration_ms, Some(5000));
    // 回合结束以后才来的：什么都不做。
    assert!(session.handle(ended(5)).is_empty());
    assert!(
        session
            .handle(delta(5, 46, Delta::End { index: 0 }))
            .is_empty()
    );
}

#[test]
fn private_data_is_kept_in_the_reply_but_not_pushed() {
    let mut session = asking();
    session.handle(sent(5));
    let private: Private =
        serde_json::from_str(r#"{"driver":"anthropic","data":{"signature":"sig"}}"#).unwrap();
    let steps = [
        Delta::Start {
            index: 0,
            kind: Kind::Reasoning,
        },
        Delta::Text {
            index: 0,
            text: "想想".to_string(),
        },
        Delta::Private {
            index: 0,
            private: private.clone(),
        },
        Delta::End { index: 0 },
    ];
    let pushed: Vec<usize> = steps
        .into_iter()
        .map(|step| session.handle(delta(5, 41, step)).len())
        .collect();
    assert_eq!(pushed, [1, 1, 0, 1], "私有数据不推");
    for piece in words(1, "好") {
        session.handle(delta(5, 42, piece));
    }
    let events = appended_events(&session.handle(ended(5)));
    let Body::MessageAssistant(reply) = &events[0].body else {
        panic!("{events:?}");
    };
    assert_eq!(
        reply.blocks[0],
        Block::Reasoning(Reasoning {
            text: "想想".to_string(),
            private: Some(private),
        })
    );
}

#[test]
fn a_reply_with_tool_calls_keeps_the_turn_open() {
    let mut session = asking();
    session.handle(sent(5));
    let call = [
        Delta::Start {
            index: 0,
            kind: Kind::ToolCall {
                name: "read".to_string(),
            },
        },
        Delta::Text {
            index: 0,
            text: r#"{"path":"src"}"#.to_string(),
        },
        Delta::End { index: 0 },
    ];
    for step in call {
        session.handle(delta(5, 41, step));
    }
    let actions = session.handle(ended(5));
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[6, 7]), "回合不结束：工具是 2-4");
    let Body::MessageAssistant(reply) = &events[0].body else {
        panic!("{events:?}");
    };
    assert!(
        matches!(&reply.blocks[..], [Block::ToolCall(call)] if call.call_id.to_string() == "call_6_1")
    );
    // 回合还开着：再来的消息是中途来的。
    let events = appended_events(&session.handle(send(2, "等一下")));
    assert_eq!((events.len(), events[0].turn), (1, Some(turn3())));
}
