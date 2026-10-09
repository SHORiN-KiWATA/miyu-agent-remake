//! 别的会话发来的话（施工 C-2，`docs/blueprint/cross-session.md` 第四条，`docs/blueprint/kernel/session.md`「别的会话发来的
//! 话」）：`by` 是会话的认三种关系，父会话的照「发一条消息」、派的子代理的照子代理的留言、别的照「别处来的」收。闲着时开一轮、
//! `trigger`、`cause` 是它；正忙时下一步听到，最后一步里到的回合结束时接着开；不带回合编号；打断两种都不撤回、不接着开；
//! 不作废在等人回答的题；没人看着的一次性会话只记下；还能恢复撤销时记在一边、载入以后恢复了接着开；撤掉它开的那一轮它留着，
//! 那一轮重做不了；发到子会话里不欠父会话回报。
//!
//! 防刷屏的三款在 `flood.rs`。

use super::reports::{CHILD, dispatched, event, last_request, trigger_of};
use super::upward::{child, last_turn};
use super::*;
use crate::event::{Question, Response};

/// 发话的会话：短编号 `22334455`。
pub(super) const PEER: &str = "0192f3a0-1111-7abc-8def-001122334455";

/// 它说一句：交回命令编号。
fn said(s: &mut Stage) -> CommandId {
    s.peer_says(PEER, "迁移写完了。")
}

/// 命令 `id` 记下的那一条 `message.user` 带的回合编号。
fn turn_of(s: &Stage, id: &CommandId) -> Option<TurnId> {
    let message = s.log().iter().find(|event| {
        event.cause.as_ref() == Some(id) && matches!(event.body, Body::MessageUser(_))
    });
    message
        .unwrap_or_else(|| panic!("{id} 没记下：{:#?}", story(s)))
        .turn
}

#[test]
fn the_three_relations_are_told_apart() {
    // 主会话正忙：派的子代理的留言、别的会话的话都是别处来的，不带回合编号；人的话排进这一轮。
    let mut s = dispatched(stage());
    s.model([Line::says("好").held(), Line::says("收到。")]);
    s.say("接着查");
    let running = Some(last_turn(&s));
    let person = s.say("还有");
    let subagent = s.child_says(CHILD, "查到了一半。");
    let peer = said(&mut s);
    assert_eq!(turn_of(&s, &person), running, "人的话排进这一轮");
    assert_eq!(turn_of(&s, &subagent), None, "子代理的留言不带回合编号");
    assert_eq!(turn_of(&s, &peer), None, "别的会话的话不带回合编号");
    // 子会话正忙：父会话的话照「发一条消息」排进这一轮；别的会话的照别处来的。
    let mut s = child();
    s.model([Line::says("好").held(), Line::says("收到。")]);
    s.say("查 A");
    let running = Some(last_turn(&s));
    let parent = s.say("再查 B");
    let peer = said(&mut s);
    assert_eq!(turn_of(&s, &parent), running, "父会话的话排进这一轮");
    assert_eq!(turn_of(&s, &peer), None, "别的会话的话不带回合编号");
}

#[test]
fn a_message_while_idle_opens_a_turn_it_triggers() {
    let mut s = stage();
    s.model([Line::says("好。")]);
    let id = said(&mut s);
    assert_eq!(
        story(&s)[1..3],
        ["2 message.user child", "3 turn.started kernel t3"],
        "不带回合编号，由它开一轮"
    );
    let message = event(&s, 2);
    assert_eq!(
        message.by,
        By::Session(crate::origin::Session {
            id: SessionId::parse(PEER).unwrap()
        })
    );
    assert_eq!(message.cause, Some(id.clone()));
    assert_eq!(trigger_of(&s, 3), Some(seq(2)));
    assert_eq!(event(&s, 3).cause, Some(id.clone()), "cause 照它的");
    assert_eq!(
        s.outcome(&id),
        Some(&Outcome::Accepted { events: seqs(&[2]) }),
        "回应只附它那一条"
    );
}

#[test]
fn a_message_while_busy_is_heard_at_the_next_step_and_opens_nothing() {
    let mut s = stage();
    s.model([
        Line::calls("我读一下。", &[("read", r#"{"path":"a"}"#)]),
        Line::says("好了。"),
    ]);
    s.tools([Play::done("A").held()]);
    s.say("看看 a");
    let running = s.ran().last().unwrap().0;
    said(&mut s);
    s.release_tool(running);
    assert_eq!(
        story(&s)[7..],
        [
            "8 message.user child",
            "9 tool.result:ok tool call_6_1 t3",
            "10 message.assistant model t3",
            "11 model.called:ok kernel t3",
            "12 turn.ended:completed kernel t3",
        ],
        "回合中途到的不带回合编号，下一次请求听到了，结束时不再开"
    );
    assert!(last_request(&s).contains("8 message.user\n"));
}

#[test]
fn a_message_in_the_last_step_opens_the_next_turn() {
    let mut s = stage();
    s.model([Line::says("好").held(), Line::says("看到了。")]);
    s.say("hi");
    let id = said(&mut s);
    s.release_model();
    assert_eq!(trigger_of(&s, 10), Some(seq(6)), "回合结束时由它接着开");
    assert_eq!(event(&s, 10).cause, Some(id));
}

#[test]
fn an_interrupt_neither_withdraws_it_nor_carries_on_with_it() {
    for queued in [Queued::Return, Queued::Send] {
        let mut s = stage();
        s.model([Line::says("").held()]);
        s.say("hi");
        said(&mut s);
        s.interrupt(queued);
        let tail: Vec<&str> = s.log()[5..].iter().map(|event| event.body.kind()).collect();
        assert_eq!(
            tail,
            ["message.user", "model.called", "turn.ended"],
            "{queued:?}：不是人说的话，不撤回，也不由它接着开"
        );
        s.model([Line::says("它说迁移写完了。")]);
        s.say("刚才谁说话了");
        assert!(last_request(&s).contains("6 message.user\n"));
    }
}

#[test]
fn it_does_not_void_a_question_waiting_for_the_person() {
    let mut s = stage();
    let questions: Vec<Question> =
        serde_json::from_str(r#"[{"question":"删掉吗？","options":[{"label":"删"}]}]"#).unwrap();
    s.model([
        Line::calls("先问你。", &[("ask_user", "{}")]),
        Line::says("好。"),
    ]);
    s.tools([Play::Asks(questions)]);
    s.say("清一清");
    said(&mut s);
    let asking = call(6, 1);
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
    assert_eq!(s.turns().len(), 1);
    assert!(
        last_request(&s).contains("9 message.user\n"),
        "答完题的下一步听到了它：{:#?}",
        story(&s)
    );
}

#[test]
fn a_oneshot_session_nobody_watches_only_records_it() {
    let mut s = Stage::oneshot(policy, environment("~/src/miyu"), at(0));
    said(&mut s);
    assert!(s.turns().is_empty(), "没人看着：只记下");
    s.watched(true);
    s.model([Line::says("看到了。")]);
    s.peer_says(PEER, "还有一件。");
    assert_eq!(s.turns().len(), 1, "有头订阅着照常开");
    assert!(
        last_request(&s).contains("2 message.user\n"),
        "只记下的一起看到"
    );
}

#[test]
fn while_an_undo_can_be_restored_it_waits_even_across_a_reload() {
    let mut s = stage();
    s.model([Line::says("好。"), Line::says("嗯。")]);
    s.say("hi");
    s.say("再来");
    s.revert(TurnId::new(seq(10)));
    said(&mut s);
    assert_eq!(s.turns().len(), 2, "还能恢复撤销：只记下");
    s.crash();
    assert_eq!(s.turns().len(), 2, "载入不因为没听到的话开轮");
    s.model([Line::says("看到了。")]);
    s.unrevert();
    assert_eq!(s.turns().len(), 3, "记在一边的照日志算回来，恢复了接着开");
    assert!(last_request(&s).contains("15 message.user\n"));
}

#[test]
fn undoing_the_turn_it_opened_keeps_it_and_that_turn_cannot_be_redone() {
    let mut s = stage();
    s.model([Line::says("好。")]);
    let id = said(&mut s);
    let opened = last_turn(&s);
    assert_eq!(event(&s, 3).cause, Some(id));
    let redo = s.redo(None);
    assert_eq!(
        s.outcome(&redo),
        Some(&Outcome::Rejected {
            reason: Reason::NotRedoable,
            about: Vec::new(),
        }),
        "它开的那一轮不是人说的话开的"
    );
    s.revert(opened);
    s.model([Line::says("它说迁移写完了。")]);
    s.say("刚才谁说话了");
    assert!(
        last_request(&s).contains("2 message.user\n"),
        "别处来的，撤销不带走：{:#?}",
        story(&s)
    );
}

#[test]
fn in_a_subagent_session_it_owes_the_parent_nothing() {
    let mut s = child();
    s.model([Line::says("查完了。"), Line::says("收到。")]);
    s.say("查 A");
    assert_eq!(s.reported_up().len(), 1);
    said(&mut s);
    assert_eq!(s.turns().len(), 2, "它叫醒了子会话");
    assert_eq!(
        s.reported_up().len(),
        1,
        "父会话没发话：它开的那一轮不向上回报"
    );
}
