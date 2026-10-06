//! 子代理的留言到了父会话（施工 7-7，`docs/blueprint/kernel/session.md`「子代理的留言」，`agents.md` 第六条第 3 条）：照回报
//! 的规矩，闲着时开一轮、`trigger` 是它；正忙时下一步听到，最后一步里到的回合结束时接着开；不带回合编号；打断时不撤回、
//! 不接着开；不作废在等人回答的题；没人看着的一次性会话只记下；派它的那一轮撤掉了的不开轮，还能恢复撤销时记在一边、恢复了
//! 接着开，载入照日志算回来；由它接着开的一轮撤掉，带走上一轮排着的话、留言留着。执行器照 [`crate::session::Session::subagents`]
//! 认 `to`。中间一层被孙代理的留言叫醒答完，不向上回报，孙代理报完了才把整件活报上去。

use super::reports::{CHILD, dispatched, event, last_request, trigger_of};
use super::upward::{GRANDCHILD, child, last_turn, up};
use super::*;
use crate::block::Block;
use crate::event::{ChildReason, Question, Response};
use crate::id::JobId;
use crate::session::Subagent;

/// 子代理 `j1` 在 13 号说了一句：交回命令编号。
fn asked(s: &mut Stage) -> CommandId {
    s.child_says(CHILD, "要改哪一个文件？")
}

#[test]
fn a_message_while_idle_opens_a_turn_it_triggers() {
    let mut s = dispatched(stage());
    s.model([Line::says("改 a.rs。")]);
    let id = asked(&mut s);
    assert_eq!(
        story(&s)[12..],
        [
            "13 message.user child",
            "14 turn.started kernel t14",
            "15 message.assistant model t14",
            "16 model.called:ok kernel t14",
            "17 turn.ended:completed kernel t14",
        ],
        "不带回合编号，由它开一轮"
    );
    let message = event(&s, 13);
    assert_eq!(
        message.by,
        By::Session(crate::origin::Session {
            id: SessionId::parse(CHILD).unwrap()
        })
    );
    assert_eq!(message.cause, Some(id.clone()));
    assert_eq!(trigger_of(&s, 14), Some(seq(13)));
    assert_eq!(event(&s, 14).cause, Some(id.clone()), "cause 照留言的");
    assert_eq!(
        s.outcome(&id),
        Some(&Outcome::Accepted {
            events: seqs(&[13])
        }),
        "回应只附留言那一条"
    );
    assert!(last_request(&s).ends_with("13 message.user\n14 turn.started\n"));
}

#[test]
fn a_message_while_busy_is_heard_at_the_next_step_and_opens_nothing() {
    let mut s = dispatched(stage());
    s.model([
        Line::calls("我读一下。", &[("read", r#"{"path":"a"}"#)]),
        Line::says("好了。"),
    ]);
    s.tools([Play::done("A").held()]);
    s.say("再看看 a");
    let running = s.ran().last().unwrap().0;
    asked(&mut s);
    s.release_tool(running);
    assert_eq!(
        story(&s)[16..],
        [
            "17 message.user child",
            "18 tool.result:ok tool call_15_1 t14",
            "19 message.assistant model t14",
            "20 model.called:ok kernel t14",
            "21 turn.ended:completed kernel t14",
        ],
        "回合中途到的不带回合编号，下一次请求听到了，结束时不再开"
    );
    let (seen, request) = s.requests().last().unwrap();
    assert_eq!(*seen, seq(18));
    assert!(listed_request(request).contains("17 message.user\n"));
}

#[test]
fn a_message_in_the_last_step_opens_the_next_turn() {
    let mut s = dispatched(stage());
    s.model([Line::says("好").held(), Line::says("改 a.rs。")]);
    s.say("hi");
    asked(&mut s);
    s.release_model();
    assert_eq!(trigger_of(&s, 19), Some(seq(15)), "回合结束时由它接着开");
    assert_eq!(event(&s, 19).cause, event(&s, 15).cause);
}

#[test]
fn an_interrupt_neither_withdraws_it_nor_carries_on_with_it() {
    for queued in [Queued::Return, Queued::Send] {
        let mut s = dispatched(stage());
        s.model([Line::says("").held()]);
        s.say("hi");
        asked(&mut s);
        s.interrupt(queued);
        let tail: Vec<&str> = s.log()[14..]
            .iter()
            .map(|event| event.body.kind())
            .collect();
        assert_eq!(
            tail,
            ["message.user", "model.called", "turn.ended"],
            "{queued:?}：不是人说的话，不撤回，也不由它接着开"
        );
        s.model([Line::says("改 a.rs。")]);
        s.say("它刚才问什么");
        assert!(last_request(&s).contains("15 message.user\n"));
    }
}

#[test]
fn it_does_not_void_a_question_waiting_for_the_person() {
    let mut s = dispatched(stage());
    let questions: Vec<Question> =
        serde_json::from_str(r#"[{"question":"删掉吗？","options":[{"label":"删"}]}]"#).unwrap();
    s.model([
        Line::calls("先问你。", &[("ask_user", "{}")]),
        Line::says("好。"),
    ]);
    s.tools([Play::Asks(questions)]);
    s.say("清一清");
    asked(&mut s);
    let asking = call(15, 1);
    s.reply(
        asking,
        vec![Response {
            picked: vec!["删".to_string()],
            text: None,
            notes: None,
        }],
    );
    let answered = s.log().iter().any(|event| {
        matches!(&event.body, Body::ToolResult(result) if result.call_id == asking
            && result.status == ToolStatus::Ok)
    });
    assert!(answered, "题还在等人：{:#?}", story(&s));
    assert_eq!(s.turns().len(), 2);
    assert!(
        last_request(&s).contains("18 message.user\n"),
        "答完题的下一步听到了留言：{:#?}",
        story(&s)
    );
}

#[test]
fn a_oneshot_session_nobody_watches_only_records_it() {
    let mut s = dispatched(Stage::oneshot(policy, environment("~/src/miyu"), at(0)));
    asked(&mut s);
    assert_eq!(s.turns().len(), 1, "没人看着：只记下");
    s.watched(true);
    s.model([Line::says("改 a.rs。")]);
    s.child_says(CHILD, "还有一个问题。");
    assert_eq!(s.turns().len(), 2, "有头订阅着照常开");
    assert!(
        last_request(&s).contains("13 message.user\n"),
        "只记下的一起看到"
    );
}

#[test]
fn a_message_from_a_subagent_of_an_undone_turn_opens_nothing() {
    let mut s = dispatched(stage());
    s.revert(TurnId::new(seq(3)));
    s.model([Line::says("好。")]);
    s.say("换个话题");
    asked(&mut s);
    assert_eq!(s.turns().len(), 2, "派它的那一轮撤掉了：只记下，不开轮");
}

#[test]
fn while_an_undo_can_be_restored_it_waits_even_across_a_reload() {
    let mut s = dispatched(stage());
    s.model([Line::says("好。")]);
    s.say("再来");
    s.revert(TurnId::new(seq(14)));
    asked(&mut s);
    assert_eq!(s.turns().len(), 2, "还能恢复撤销：只记下");
    s.crash();
    assert_eq!(s.turns().len(), 2, "载入不因为没听到的留言开轮");
    s.model([Line::says("改 a.rs。")]);
    s.unrevert();
    assert_eq!(s.turns().len(), 3, "记在一边的照日志算回来，恢复了接着开");
    assert!(last_request(&s).contains("19 message.user\n"));
}

#[test]
fn undoing_a_turn_it_carried_on_takes_the_queued_words_and_keeps_the_message() {
    let mut s = dispatched(stage());
    s.model([Line::says("好").held(), Line::says("都看到了。")]);
    s.say("hi");
    s.say("还有这句");
    asked(&mut s);
    s.release_model();
    assert_eq!(trigger_of(&s, 20), Some(seq(16)), "留言接着开了下一轮");
    s.revert(TurnId::new(seq(20)));
    s.model([Line::says("嗯。")]);
    s.say("换个话题");
    let request = last_request(&s);
    assert!(
        !request.contains("15 message.user\n"),
        "那一轮才听到的人的话跟着撤：{request}"
    );
    assert!(
        request.contains("16 message.user\n"),
        "留言是别处来的，留着：{request}"
    );
}

#[test]
fn after_a_reload_the_next_turn_hears_a_recorded_message() {
    let mut s = dispatched(Stage::oneshot(policy, environment("~/src/miyu"), at(0)));
    asked(&mut s);
    s.crash();
    assert_eq!(s.turns().len(), 1, "载入不因为没听到的留言开轮");
    s.model([Line::says("改 a.rs。")]);
    let id = s.say("它问了什么");
    let opened = s.turns()[1];
    assert_eq!(
        event(&s, opened.started().get()).cause,
        Some(id),
        "由人的那句开"
    );
    assert!(last_request(&s).contains("13 message.user\n"));
}

#[test]
fn the_executor_is_given_the_subagents_she_can_message() {
    let mut s = stage();
    s.model([
        Line::calls("派出去。", &[("read", "{}"); 4]),
        Line::says("派出去了。"),
    ]);
    let other = "01a0d78c-ca52-7d19-8b64-000000000003";
    let stopped = "01a0d78c-ca52-7d19-8b64-000000000004";
    s.tools([
        Play::starts_agent(1, "a", CHILD),
        Play::starts_command(2, "b"),
        Play::starts_agent(3, "c", other),
        Play::starts_agent(4, "d", stopped),
    ]);
    s.say("派四个");
    s.model([Line::says("看到了。"), Line::says("停了。")]);
    s.child_reports(3, other, ChildReason::Done, "做完了。");
    s.child_reports(4, stopped, ChildReason::Stopped, "");
    let subagent = |session: &str, stopped| Subagent {
        session: SessionId::parse(session).unwrap(),
        stopped,
    };
    let job = |n| JobId::new(n).unwrap();
    assert_eq!(
        s.subagents(),
        [
            (job(1), subagent(CHILD, false)),
            (job(3), subagent(other, false)),
            (job(4), subagent(stopped, true)),
        ]
        .into(),
        "后台命令不在；做完了报过的照样在，被停掉的标着"
    );
    s.revert(TurnId::new(seq(3)));
    assert!(
        s.subagents().is_empty(),
        "派它的那一轮撤掉了：她看不到，就不是她的"
    );
    s.unrevert();
    assert_eq!(s.subagents().len(), 3);
}

#[test]
fn the_middle_layer_answers_its_subagent_and_reports_the_whole_task_after() {
    let mut s = child();
    s.model([
        Line::calls("派孙代理。", &[("read", "{}")]),
        Line::says("等它。"),
        Line::says("用 a.rs。"),
        Line::says("A 查完了，B 也在里面。"),
        Line::says("再答一次。"),
    ]);
    s.tools([Play::starts_agent(1, "查 B", GRANDCHILD)]);
    s.say("查 A");
    assert!(s.reported_up().is_empty(), "孙代理还没报：不报");
    s.child_says(GRANDCHILD, "要改哪一个文件？");
    assert_eq!(s.turns().len(), 2, "孙代理的留言叫醒它");
    assert!(
        s.reported_up().is_empty(),
        "由孙代理的留言开的那一轮答完了，孙代理还没报：不报"
    );
    s.child_reports(1, GRANDCHILD, ChildReason::Done, "B 查完了。");
    assert_eq!(
        s.reported_up(),
        [up(
            last_turn(&s),
            ChildReason::Done,
            "A 查完了，B 也在里面。",
            false,
            false
        )],
        "孙代理报完、被叫醒的那一轮结束，才把整件活报上去"
    );
    s.child_says(GRANDCHILD, "还有一个问题。");
    assert_eq!(s.turns().len(), 4);
    assert_eq!(
        s.reported_up().len(),
        1,
        "父会话没再发话：孙代理的留言开的那一轮不向上回报"
    );
}

#[test]
fn a_message_from_a_session_she_did_not_start_is_from_another_session() {
    let mut s = dispatched(stage());
    s.model([Line::says("好").held(), Line::says("嗯。")]);
    s.say("hi");
    s.child_says("01a0d78c-ca52-7d19-8b64-000000000009", "我是谁？");
    s.release_model();
    let message = event(&s, 15);
    assert_eq!(
        message.turn, None,
        "不是她派的：别的会话发来的话（施工 C-2），照别处来的收，不带回合编号"
    );
    assert!(
        matches!(&message.body, Body::MessageUser(user) if user.blocks
        == [Block::Text(crate::block::Text { text: "我是谁？".to_string() })])
    );
}

#[test]
fn a_subagent_she_answered_owes_her_a_report_before_she_reports_up() {
    let mut s = child();
    s.model([
        Line::calls("派孙代理。", &[("read", "{}")]),
        Line::says("等它。"),
        Line::calls("答它。", &[("read", "{}")]),
        Line::says("告诉它了，等它做完。"),
        Line::says("A 查完了，B 也在里面。"),
    ]);
    s.tools([Play::starts_agent(1, "查 B", GRANDCHILD), Play::messages(1)]);
    s.say("查 A");
    // 孙代理停下来在回报里问：报过一次了，叫醒它，它留言答复。
    s.child_reports(1, GRANDCHILD, ChildReason::Done, "改哪一个文件？我等着。");
    assert_eq!(s.turns().len(), 2);
    assert!(
        s.reported_up().is_empty(),
        "留了言的孙代理欠一份回报：答完这一轮不向上回报"
    );
    s.child_reports(1, GRANDCHILD, ChildReason::Done, "B 查完了。");
    assert_eq!(
        s.reported_up(),
        [up(
            last_turn(&s),
            ChildReason::Done,
            "A 查完了，B 也在里面。",
            false,
            false
        )],
        "孙代理把留言的那件也报了，才把整件活报上去"
    );
}
