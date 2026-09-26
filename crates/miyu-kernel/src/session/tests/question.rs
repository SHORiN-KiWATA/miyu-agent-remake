//! 提问（`docs/designs/02-内核.md` 第六节「提问怎么走」）：在跑的调用问人；回答落了盘交给工具；
//! 答完还能再问；回答被拒的几种；没人能回答；打断；等人回答时来了一句话，提问和确认都作废；
//! 工具自己先交回结果；过时的提问。

use super::approval::answer;
use super::executor::*;
use super::*;
use crate::event::{Choice, Decision, Question, Response, ToolStatus};
use crate::id::{CallId, ModuleId};
use crate::origin::Tool;
use crate::tool::Access;

/// 编号是 `n` 的命令：alice 回答 `call_id` 问的那组题。
pub(super) fn reply(n: u64, call_id: CallId, answers: Vec<Response>) -> Input {
    Input::Command(Received {
        id: id(n),
        by: alice(),
        at: at(n % 60),
        command: Command::Answer {
            call_id,
            answer: Answer::Questions(answers),
        },
    })
}

/// 一道单选题：旧的 build 目录删不删。
pub(super) fn build_question() -> Vec<Question> {
    vec![Question {
        header: Some("build".to_string()),
        question: "旧的 build 目录要删掉，还是保留？".to_string(),
        options: ["删掉", "保留"]
            .iter()
            .map(|label| Choice {
                label: label.to_string(),
                description: None,
            })
            .collect(),
        multiple: false,
    }]
}

pub(super) fn picked(labels: &[&str]) -> Response {
    Response {
        picked: labels.iter().map(|label| label.to_string()).collect(),
        text: None,
    }
}

/// 请求 5 号调了这几件工具（回复 6 号、`model.called` 7 号），落了盘，链都放行，都在跑。
fn running(calls: &[(&str, &str)]) -> Session {
    let mut session = asking();
    call_tools(&mut session, 5, calls);
    allowing(&mut session, stored(7));
    session
}

/// 同上，只有一件问人的，它问了那道 build 的题（8 号），落了盘。
fn asked() -> Session {
    let mut session = running(&[("ask_user", "{}")]);
    session.handle(asks(call(6, 1), build_question()));
    session.handle(stored(8));
    session
}

#[test]
fn a_running_call_asks_and_waits_for_you() {
    let mut session = running(&[("ask_user", "{}")]);
    let actions = session.handle(asks(call(6, 1), build_question()));
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[8]));
    let Body::QuestionAsked(asked) = &events[0].body else {
        panic!("{events:?}");
    };
    assert_eq!(
        (asked.call_id, &asked.questions),
        (call(6, 1), &build_question())
    );
    assert_eq!(
        events[0].by,
        By::Tool(Tool {
            call_id: call(6, 1)
        })
    );
    assert_eq!(events[0].cause, Some(id(1)));
    let actions = session.handle(stored(8));
    assert!(
        handed(&actions).is_empty() && calls(&actions).is_empty(),
        "等你回答"
    );
}

#[test]
fn the_answer_goes_to_the_tool_once_it_is_stored() {
    let mut session = asked();
    let actions = session.handle(reply(2, call(6, 1), vec![picked(&["保留"])]));
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[9]));
    let Body::QuestionAnswered(answered) = &events[0].body else {
        panic!("{events:?}");
    };
    assert_eq!(answered.answers, [picked(&["保留"])]);
    assert_eq!(
        (events[0].by.clone(), events[0].cause.clone()),
        (alice(), Some(id(2)))
    );
    assert!(handed(&actions).is_empty(), "回答落了盘才交给工具");
    let actions = session.handle(stored(9));
    assert_eq!(handed(&actions), [(call(6, 1), vec![picked(&["保留"])])]);
    assert!(actions.contains(&accepted_reply(2, &[9])));
    // 工具照回答写成结果，这一步照常往下走。
    let actions = session.handle(done(call(6, 1), "kept"));
    assert_eq!(appended(&actions), seqs(&[10]));
    assert_eq!(
        calls(&session.handle(stored(10)))
            .first()
            .map(|(seen, _)| *seen),
        Some(seq(10))
    );
}

#[test]
fn an_answer_is_not_handed_over_before_it_is_stored() {
    let mut session = running(&[("ask_user", "{}"), ("read", "{}")]);
    session.handle(asks(call(6, 1), build_question()));
    session.handle(stored(8));
    session.handle(reply(2, call(6, 1), vec![picked(&["保留"])]));
    // 回答还没落盘时，别的调用跑完了、要派后面的：回答照样等它落了盘。
    let actions = session.handle(done(call(6, 2), "a"));
    assert!(handed(&actions).is_empty(), "回答还没落盘");
    assert_eq!(
        handed(&session.handle(stored(10))),
        [(call(6, 1), vec![picked(&["保留"])])]
    );
}

#[test]
fn a_call_can_ask_again_after_an_answer() {
    let mut session = asked();
    session.handle(reply(2, call(6, 1), vec![picked(&["删掉"])]));
    session.handle(stored(9));
    let actions = session.handle(asks(call(6, 1), build_question()));
    assert_eq!(appended(&actions), seqs(&[10]), "又问了一组");
    session.handle(stored(10));
    let with_text = Response {
        picked: Vec::new(),
        text: Some("先问问我同事".to_string()),
    };
    assert_eq!(
        appended(&session.handle(reply(3, call(6, 1), vec![with_text]))),
        seqs(&[11])
    );
}

#[test]
fn answers_that_do_not_fit_are_rejected() {
    let mut session = asked();
    let two = vec![picked(&["删掉"]), picked(&[])];
    let cases = [
        (reply(2, call(6, 1), two), Reason::BadAnswer),
        (
            reply(3, call(6, 1), vec![picked(&["全删"])]),
            Reason::BadAnswer,
        ),
        (
            reply(4, call(6, 1), vec![picked(&["删掉", "保留"])]),
            Reason::BadAnswer,
        ),
        (
            reply(5, call(6, 1), vec![picked(&["保留", "保留"])]),
            Reason::BadAnswer,
        ),
        (
            reply(6, call(6, 2), vec![picked(&["保留"])]),
            Reason::NotAsking,
        ),
        (
            answer(7, call(6, 1), Decision::Once, None),
            Reason::NotAsking,
        ),
    ];
    for (input, reason) in cases {
        let Input::Command(Received { id: ref n, .. }) = input else {
            unreachable!()
        };
        let n = n.clone();
        assert_eq!(
            session.handle(input),
            [Action::Reply {
                id: n,
                outcome: Outcome::Rejected { reason },
            }]
        );
    }
    assert_eq!(Reason::BadAnswer.code(), "bad_answer");
    // 空的自己写当没写；先答者胜，答过了再答，拒绝。
    let blank = Response {
        picked: vec!["保留".to_string()],
        text: Some("  ".to_string()),
    };
    let events = appended_events(&session.handle(reply(8, call(6, 1), vec![blank])));
    let Body::QuestionAnswered(answered) = &events[0].body else {
        panic!("{events:?}");
    };
    assert_eq!(answered.answers, [picked(&["保留"])]);
    assert_eq!(
        session.handle(reply(9, call(6, 1), vec![picked(&["删掉"])])),
        [Action::Reply {
            id: id(9),
            outcome: Outcome::Rejected {
                reason: Reason::NotAsking
            },
        }]
    );
}

#[test]
fn where_no_one_can_answer_the_question_is_skipped() {
    let mut policy = policy();
    policy.attended = false;
    let mut session = session_with(policy);
    session.handle(send(1, "hi"));
    session.handle(stored(5));
    session.handle(hooks_done(turn3(), Vec::new()));
    call_tools(&mut session, 5, &[("ask_user", "{}")]);
    allowing(&mut session, stored(7));
    let actions = session.handle(asks(call(6, 1), build_question()));
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[8]), "不记题目");
    assert_eq!(
        result_of(&events[0]),
        (
            call(6, 1),
            ToolStatus::Skipped,
            By::Kernel,
            "question unattended".to_string()
        )
    );
    assert_eq!(stopped(&actions), [call(6, 1)]);
    assert!(
        session.handle(done(call(6, 1), "late")).is_empty(),
        "之后的结果不理"
    );
}

#[test]
fn interrupting_while_you_are_asked_stops_the_call() {
    let mut session = asked();
    let actions = session.handle(interrupt(2));
    let events = appended_events(&actions);
    assert_eq!(
        result_of(&events[0]),
        (
            call(6, 1),
            ToolStatus::Cancelled,
            alice(),
            "question interrupted".to_string()
        )
    );
    assert_eq!(stopped(&actions), [call(6, 1)]);
    assert!(session.handle(done(call(6, 1), "late")).is_empty());
    // 答完了、还没交给工具时打断：照在跑的算。
    let mut session = asked();
    session.handle(reply(2, call(6, 1), vec![picked(&["保留"])]));
    let events = appended_events(&session.handle(interrupt(3)));
    assert_eq!(result_of(&events[0]).3, "cancelled running");
}

#[test]
fn a_message_while_you_are_asked_voids_the_question() {
    for urgent_or_not in [send(2, "先看测试"), urgent(2, "先看测试")] {
        let mut session = running(&[("ask_user", "{}"), ("read", "{}")]);
        session.handle(asks(call(6, 1), build_question()));
        session.handle(stored(8));
        let actions = session.handle(urgent_or_not);
        let events = appended_events(&actions);
        assert_eq!(appended(&actions), seqs(&[9, 10]), "消息，作废的结果");
        assert_eq!(
            result_of(&events[1]),
            (
                call(6, 1),
                ToolStatus::Skipped,
                alice(),
                "question voided".to_string()
            )
        );
        assert_eq!(events[1].cause, Some(id(2)));
        assert_eq!(stopped(&actions), [call(6, 1)]);
        // 别的在跑的照常跑完；这一步齐了，这句话在下一次请求里。
        let actions = session.handle(done(call(6, 2), "a"));
        assert_eq!(appended(&actions), seqs(&[11]));
        let actions = session.handle(stored(11));
        let (seen, request) = calls(&actions).remove(0);
        assert_eq!(seen, seq(11));
        assert!(request.contains("9 message.user"), "{request}");
    }
}

#[test]
fn a_message_while_you_are_asked_to_approve_skips_the_call() {
    let mut session = asking();
    call_tools(&mut session, 5, &[("write", "{}")]);
    session.handle(stored(7));
    let ask = Verdict::Ask {
        module: ModuleId::parse("permissions").unwrap(),
        access: Access::Write,
        rule: None,
        detail: None,
    };
    session.handle(guarded(call(6, 1), ask));
    session.handle(stored(8));
    let actions = session.handle(send(2, "别写了"));
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[9, 10]));
    assert_eq!(
        result_of(&events[1]),
        (
            call(6, 1),
            ToolStatus::Skipped,
            alice(),
            "skipped".to_string()
        )
    );
    assert!(stopped(&actions).is_empty(), "确认的调用没跑过，不用叫停");
    assert_eq!(
        calls(&session.handle(stored(10)))
            .first()
            .map(|(seen, _)| *seen),
        Some(seq(10))
    );
}

#[test]
fn the_tool_finishing_first_closes_the_question() {
    let mut session = asked();
    assert_eq!(
        appended(&session.handle(done(call(6, 1), "gave up"))),
        seqs(&[9])
    );
    assert_eq!(
        session.handle(reply(2, call(6, 1), vec![picked(&["保留"])])),
        [Action::Reply {
            id: id(2),
            outcome: Outcome::Rejected {
                reason: Reason::NotAsking
            },
        }]
    );
}

#[test]
fn questions_from_calls_not_running_are_ignored() {
    let mut session = asking();
    call_tools(&mut session, 5, &[("ask_user", "{}")]);
    assert!(
        session
            .handle(asks(call(6, 1), build_question()))
            .is_empty(),
        "还没派"
    );
    let mut session = asked();
    assert!(
        session
            .handle(asks(call(6, 1), build_question()))
            .is_empty(),
        "已经在问了"
    );
    assert!(
        session
            .handle(asks(call(6, 2), build_question()))
            .is_empty(),
        "没有这个调用"
    );
}
