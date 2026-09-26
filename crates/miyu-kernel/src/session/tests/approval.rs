//! 确认（`docs/designs/02-内核.md` 第六节「确认怎么走」）：先过链；链的三种结论；等结论、等人的
//! 占着位置；人允许、人拒绝，拒绝以后她接着干；回答被拒的几种；没人能确认的会话；收紧成只读；
//! 打断、急着插话；过时的结论。

use super::executor::*;
use super::*;
use crate::event::{ApprovalDecided, ApprovalRequested, Decision, Level, Permission, ToolStatus};
use crate::id::{CallId, ModuleId};
use crate::origin::Module;
use crate::raw::RawJson;
use crate::tool::Access;

/// 编号是 `n` 的命令：alice 回答 `call_id` 的确认。
pub(super) fn answer(n: u64, call_id: CallId, decision: Decision, reason: Option<&str>) -> Input {
    Input::Command(Received {
        id: id(n),
        by: alice(),
        at: at(n % 60),
        command: Command::Answer {
            call_id,
            answer: Answer::Approval {
                decision,
                reason: reason.map(str::to_string),
            },
        },
    })
}

/// 替身的权限策略。
fn permissions() -> ModuleId {
    ModuleId::parse("permissions").unwrap()
}

fn raw(text: &str) -> RawJson {
    serde_json::from_str(text).unwrap()
}

/// 链说要问人：要的是 `access`；`rule` 是提没提放行规则。
fn ask(access: Access, rule: bool) -> Verdict {
    Verdict::Ask {
        module: permissions(),
        access,
        rule: rule.then(|| raw(r#"{"path":"~/x"}"#)),
        detail: Some(raw(r#"{"reason":"outside_workspace"}"#)),
    }
}

fn denied_by_chain() -> Verdict {
    Verdict::Deny {
        module: permissions(),
        text: "blocked".to_string(),
    }
}

fn request_of(event: &Event) -> &ApprovalRequested {
    match &event.body {
        Body::ApprovalRequested(request) => request,
        body => panic!("应该是 tool.approval_requested：{body:?}"),
    }
}

fn decision_of(event: &Event) -> &ApprovalDecided {
    match &event.body {
        Body::ApprovalDecided(decided) => decided,
        body => panic!("应该是 tool.approval_decided：{body:?}"),
    }
}

fn rejected_with(reason: Reason, n: u64) -> Vec<Action> {
    vec![Action::Reply {
        id: id(n),
        outcome: Outcome::Rejected { reason },
    }]
}

/// 请求 5 号调了这几件工具（回复 6 号、`model.called` 7 号），落了盘：返回会话和交给链的调用。
fn guarding(calls: &[(&str, &str)]) -> (Session, Vec<CallId>) {
    let mut session = asking();
    call_tools(&mut session, 5, calls);
    let guarded = guards(&session.handle(stored(7)));
    (session, guarded)
}

/// 同上，调一件写文件的，链说要问人（请求是 8 号，落了盘）。
fn waiting_for_you(rule: bool) -> Session {
    let (mut session, _) = guarding(&[("write", "{}")]);
    session.handle(guarded(call(6, 1), ask(Access::Write, rule)));
    session.handle(stored(8));
    session
}

#[test]
fn every_call_goes_through_the_chain_before_it_runs() {
    let mut session = asking();
    call_tools(&mut session, 5, &[("read", r#"{"limit":"5"}"#)]);
    let actions = session.handle(stored(7));
    assert_eq!(
        actions.last(),
        Some(&Action::GuardTool {
            call_id: call(6, 1),
            name: "read".to_string(),
            args: r#"{"limit":5}"#.to_string(),
            cwd: "~/src/miyu".to_string(),
            permission: Permission {
                level: Level::Workspace,
                read_only: false,
            },
        })
    );
    assert!(ran(&actions).is_empty(), "链放行了才派");
    let actions = session.handle(guarded(call(6, 1), Verdict::Allow));
    assert_eq!(ran(&actions), [call(6, 1)]);
    assert!(appended(&actions).is_empty(), "放行的不记事件");
}

#[test]
fn the_chain_is_told_the_level_in_effect() {
    use super::permission::read_only;
    let mut session = session();
    session.handle(read_only(1, true));
    session.handle(send(2, "hi"));
    session.handle(stored(6));
    session.handle(hooks_done(TurnId::new(seq(4)), Vec::new()));
    // 请求在路上时关了只读：放宽的等下一次请求，这一步交给链的还是只读。
    session.handle(read_only(3, false));
    call_tools(&mut session, 6, &[("read", "{}")]);
    let actions = session.handle(stored(9));
    let Some(Action::GuardTool { permission, .. }) = actions.last() else {
        panic!("{actions:?}");
    };
    assert!(permission.read_only, "{permission:?}");
}

#[test]
fn a_denial_from_the_chain_is_recorded_by_the_module() {
    let (mut session, _) = guarding(&[("write", "{}")]);
    let actions = session.handle(guarded(call(6, 1), denied_by_chain()));
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[8]));
    assert_eq!(
        result_of(&events[0]),
        (
            call(6, 1),
            ToolStatus::Denied,
            By::Module(Module { id: permissions() }),
            "blocked".to_string()
        )
    );
    assert_eq!(
        (events[0].cause.clone(), events[0].turn),
        (Some(id(1)), Some(turn3()))
    );
    // 这一步齐了，照常请求下一次。
    assert_eq!(
        calls(&session.handle(stored(8)))
            .first()
            .map(|(seen, _)| *seen),
        Some(seq(8))
    );
}

#[test]
fn asking_records_a_request_and_the_call_waits() {
    let (mut session, _) = guarding(&[("write", "{}")]);
    let actions = session.handle(guarded(call(6, 1), ask(Access::Write, true)));
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[8]));
    let request = request_of(&events[0]);
    assert_eq!(
        (request.call_id, &request.access),
        (call(6, 1), &Access::Write)
    );
    assert_eq!(
        request.rule.as_ref().map(RawJson::get),
        Some(r#"{"path":"~/x"}"#)
    );
    assert!(request.detail.is_some());
    assert_eq!(events[0].by, By::Module(Module { id: permissions() }));
    assert_eq!(events[0].cause, Some(id(1)));
    let actions = session.handle(stored(8));
    assert!(
        ran(&actions).is_empty() && calls(&actions).is_empty(),
        "等你回答"
    );
}

#[test]
fn calls_waiting_for_the_chain_or_for_you_hold_their_place() {
    // 写文件的在等人，后面读的不越过它。
    let (mut session, guarded_now) = guarding(&[("write", "{}"), ("read", "{}")]);
    assert_eq!(guarded_now, [call(6, 1)]);
    let actions = session.handle(guarded(call(6, 1), ask(Access::Write, true)));
    assert!(guards(&actions).is_empty(), "链说要问人的那一刻");
    assert!(guards(&session.handle(stored(8))).is_empty(), "请求落了盘");
    // 两个读的一起过链：一个在等人，另一个照跑。
    let (mut session, guarded_now) = guarding(&[("read", "{}"), ("read", "{}")]);
    assert_eq!(guarded_now, [call(6, 1), call(6, 2)]);
    session.handle(guarded(call(6, 1), ask(Access::Read, false)));
    let actions = session.handle(guarded(call(6, 2), Verdict::Allow));
    assert_eq!(ran(&actions), [call(6, 2)]);
}

#[test]
fn allowing_runs_the_call_once_the_decision_is_stored() {
    for decision in [Decision::Once, Decision::Session, Decision::Workspace] {
        let mut session = waiting_for_you(true);
        let actions = session.handle(answer(2, call(6, 1), decision.clone(), None));
        let events = appended_events(&actions);
        assert_eq!(appended(&actions), seqs(&[9]));
        let decided = decision_of(&events[0]);
        assert_eq!(
            (decided.call_id, &decided.decision),
            (call(6, 1), &decision)
        );
        assert_eq!(decided.reason, None);
        assert_eq!(
            (events[0].by.clone(), events[0].cause.clone()),
            (alice(), Some(id(2)))
        );
        assert!(ran(&actions).is_empty(), "决定落了盘才派");
        let actions = session.handle(stored(9));
        assert_eq!(ran(&actions), [call(6, 1)]);
        assert!(actions.contains(&accepted_reply(2, &[9])));
    }
}

#[test]
fn denying_records_the_decision_and_the_result_and_she_goes_on() {
    for (reason, text) in [
        (None, "denied"),
        (Some("先别推"), "denied: 先别推"),
        (Some("  "), "denied"),
    ] {
        let mut session = waiting_for_you(true);
        let actions = session.handle(answer(2, call(6, 1), Decision::Deny, reason));
        let events = appended_events(&actions);
        assert_eq!(appended(&actions), seqs(&[9, 10]));
        let written = reason.filter(|reason| !reason.trim().is_empty());
        assert_eq!(
            decision_of(&events[0]).reason.as_deref(),
            written,
            "空的理由当没写"
        );
        assert_eq!(
            result_of(&events[1]),
            (call(6, 1), ToolStatus::Denied, alice(), text.to_string())
        );
        assert_eq!(events[1].cause, Some(id(2)));
        // 她接着干：这一步齐了，请求下一次。
        let actions = session.handle(stored(10));
        assert_eq!(
            calls(&actions).first().map(|(seen, _)| *seen),
            Some(seq(10))
        );
        assert!(actions.contains(&accepted_reply(2, &[9, 10])));
    }
}

#[test]
fn answers_that_do_not_fit_are_rejected() {
    let mut session = waiting_for_you(false);
    let asked = call(6, 1);
    let cases = [
        (
            answer(2, call(6, 2), Decision::Once, None),
            Reason::NotAsking,
        ),
        (
            answer(3, asked, Decision::Other("maybe".to_string()), None),
            Reason::UnknownDecision,
        ),
        (answer(4, asked, Decision::Session, None), Reason::NoRule),
        (answer(5, asked, Decision::Workspace, None), Reason::NoRule),
        (
            answer(6, asked, Decision::Once, Some("好")),
            Reason::UnexpectedReason,
        ),
    ];
    for (input, reason) in cases {
        let Input::Command(Received { id: ref n, .. }) = input else {
            unreachable!()
        };
        let n = n.clone();
        assert_eq!(
            session.handle(input),
            vec![Action::Reply {
                id: n,
                outcome: Outcome::Rejected { reason },
            }]
        );
    }
    let codes: Vec<&str> = [
        Reason::NotAsking,
        Reason::UnknownDecision,
        Reason::NoRule,
        Reason::UnexpectedReason,
    ]
    .iter()
    .map(|reason| reason.code())
    .collect();
    assert_eq!(
        codes,
        [
            "not_asking",
            "unknown_decision",
            "no_rule",
            "unexpected_reason"
        ]
    );
    // 被拒过的回答什么都没记；答对了以后，同一个编号再来照上一次回应，再答一次就不在等了。
    assert_eq!(
        appended(&session.handle(answer(7, asked, Decision::Once, None))),
        seqs(&[9])
    );
    session.handle(stored(9));
    assert_eq!(
        session.handle(answer(7, asked, Decision::Once, None)),
        [accepted_reply(7, &[9])]
    );
    assert_eq!(
        session.handle(answer(8, asked, Decision::Deny, None)),
        rejected_with(Reason::NotAsking, 8)
    );
}

#[test]
fn where_no_one_can_approve_asking_is_denied_on_the_spot() {
    let mut policy = policy();
    policy.attended = false;
    let mut session = session_with(policy);
    session.handle(send(1, "hi"));
    session.handle(stored(5));
    session.handle(hooks_done(turn3(), Vec::new()));
    call_tools(&mut session, 5, &[("write", "{}")]);
    session.handle(stored(7));
    let actions = session.handle(guarded(call(6, 1), ask(Access::Write, true)));
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[8]), "不记请求");
    assert_eq!(
        result_of(&events[0]),
        (
            call(6, 1),
            ToolStatus::Denied,
            By::Kernel,
            "unattended".to_string()
        )
    );
    assert_eq!(
        calls(&session.handle(stored(8)))
            .first()
            .map(|(seen, _)| *seen),
        Some(seq(8))
    );
}

#[test]
fn tightening_to_read_only_denies_what_would_write() {
    use super::permission::read_only;
    // 在过链的写文件调用：当场拦下。
    let (mut session, _) = guarding(&[("write", "{}")]);
    let events = appended_events(&session.handle(read_only(2, true)));
    assert_eq!(
        result_of(&events[1]),
        (
            call(6, 1),
            ToolStatus::Denied,
            By::Kernel,
            "read only".to_string()
        )
    );
    assert!(
        session
            .handle(guarded(call(6, 1), Verdict::Allow))
            .is_empty(),
        "迟到的结论不理"
    );
    // 在等人、要的是写入的：当场拦下，请求就了结了。
    let (mut session, _) = guarding(&[("shell", "{}")]);
    session.handle(guarded(call(6, 1), ask(Access::Write, true)));
    let events = appended_events(&session.handle(read_only(2, true)));
    assert_eq!(result_of(&events[1]).1, ToolStatus::Denied);
    assert_eq!(
        session.handle(answer(3, call(6, 1), Decision::Once, None)),
        rejected_with(Reason::NotAsking, 3)
    );
    // 只读以后才到的结论要写入：不记请求，当场拦下。
    let (mut session, _) = guarding(&[("shell", "{}")]);
    session.handle(read_only(2, true));
    let actions = session.handle(guarded(call(6, 1), ask(Access::Write, true)));
    let events = appended_events(&actions);
    assert_eq!(events.len(), 2, "拦下的结果，和这一步齐了以后的权限事实");
    assert_eq!(result_of(&events[0]).2, By::Kernel);
    // 要读的请求照旧等，回答了照样派。
    let (mut session, _) = guarding(&[("read", "{}")]);
    session.handle(guarded(call(6, 1), ask(Access::Read, false)));
    assert_eq!(appended(&session.handle(read_only(2, true))), seqs(&[9]));
    session.handle(answer(3, call(6, 1), Decision::Once, None));
    assert_eq!(ran(&session.handle(stored(10))), [call(6, 1)]);
}

#[test]
fn interrupting_cancels_calls_waiting_for_the_chain_or_for_you() {
    let (mut session, _) = guarding(&[("read", "{}"), ("read", "{}")]);
    session.handle(guarded(call(6, 1), ask(Access::Read, true)));
    let actions = session.handle(interrupt(2));
    let events = appended_events(&actions);
    for (k, call_id) in [(0, call(6, 1)), (1, call(6, 2))] {
        assert_eq!(
            result_of(&events[k]),
            (
                call_id,
                ToolStatus::Cancelled,
                alice(),
                "cancelled before".to_string()
            )
        );
    }
    assert!(
        !actions
            .iter()
            .any(|action| matches!(action, Action::CancelTool { .. }))
    );
    assert!(
        session
            .handle(guarded(call(6, 2), Verdict::Allow))
            .is_empty(),
        "迟到的结论不理"
    );
    assert_eq!(
        session.handle(answer(3, call(6, 1), Decision::Once, None)),
        rejected_with(Reason::NotAsking, 3)
    );
}

#[test]
fn an_urgent_message_skips_calls_waiting_for_the_chain_or_for_you() {
    let (mut session, _) = guarding(&[("read", "{}"), ("read", "{}")]);
    session.handle(guarded(call(6, 1), ask(Access::Read, true)));
    let actions = session.handle(urgent(2, "等等"));
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[9, 10, 11]));
    for (k, call_id) in [(1, call(6, 1)), (2, call(6, 2))] {
        assert_eq!(
            result_of(&events[k]),
            (call_id, ToolStatus::Skipped, alice(), "skipped".to_string())
        );
    }
    assert_eq!(
        calls(&session.handle(stored(11)))
            .first()
            .map(|(seen, _)| *seen),
        Some(seq(11))
    );
}

#[test]
fn verdicts_for_calls_not_waiting_on_the_chain_are_ignored() {
    let (mut session, _) = guarding(&[("read", "{}")]);
    assert!(
        session
            .handle(guarded(call(6, 2), Verdict::Allow))
            .is_empty(),
        "没有这个调用"
    );
    session.handle(guarded(call(6, 1), Verdict::Allow));
    assert!(
        session
            .handle(guarded(call(6, 1), denied_by_chain()))
            .is_empty(),
        "已经在跑了"
    );
}
