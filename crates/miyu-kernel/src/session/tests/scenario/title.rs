//! 场景：起标题（施工 3-8 五补，`docs/blueprint/kernel/session.md`「起标题」）。没起名的会话一轮答完，内核单独发一次请求，
//! 说完了记一条带 `purpose` 的 `model.called` 和 `session.meta_changed`，都是内核记的、不带回合编号、没有 `cause`；标题取第一行、
//! 超了上限截掉；人起过名的、子会话、一次性的会话不起；没答出正文的那一轮不起；失败了下一轮答完再试，一共两次；在路上时人
//! 改了名的不盖掉；一次只有一个在路上；在路上的跟着重启丢了的不算。
//!
//! 替身的组装一条事件一行（[`Listing`]）：起标题的请求是第一个回答和它前面人的话的清单，最后一条写着「title」。

use super::upward::PARENT;
use super::*;
use crate::event::{EndReason, MetaChanged, ModelCalled, Purpose};
use crate::session::Titles;

/// 起标题的策略：试两次，标题最多 50 个字。
fn titled() -> Policy {
    Policy {
        titles: Some(Titles {
            tries: 2,
            chars: 50,
        }),
        ..policy()
    }
}

/// 照 [`titled`] 造的会话的替身。
fn titled_stage() -> Stage {
    Stage::new(titled, environment("~/src/miyu"), at(0))
}

/// 日志里的 `session.meta_changed`，照先后：谁记的、写的两格。
fn renamed(stage: &Stage) -> Vec<(By, MetaChanged)> {
    stage
        .log()
        .iter()
        .filter_map(|event| match &event.body {
            Body::MetaChanged(changed) => Some((event.by.clone(), changed.clone())),
            _ => None,
        })
        .collect()
}

/// 内核起的标题。
fn kernel_titled(title: &str) -> (By, MetaChanged) {
    (
        By::Kernel,
        MetaChanged {
            title: Some(title.to_string()),
            pinned: None,
        },
    )
}

/// 起标题的 `model.called`，照先后。
fn title_calls(stage: &Stage) -> Vec<&ModelCalled> {
    stage
        .model_calls()
        .into_iter()
        .filter(|called| called.purpose == Some(Purpose::Title))
        .collect()
}

/// 说一句，她答一句就完。
fn one_turn(stage: &mut Stage, words: &str) {
    stage.model([Line::says("好。")]);
    stage.say(words);
}

#[test]
fn the_first_answer_gets_a_title_recorded_by_the_kernel_outside_any_turn() {
    let mut stage = titled_stage();
    stage.title_model([Line::says("  打个招呼\n第二行不要\n").thinking("想想")]);
    one_turn(&mut stage, "hi");
    assert_eq!(
        story(&stage)[7..],
        [
            "8 turn.ended:completed kernel t3",
            "9 model.called:ok kernel",
            "10 session.meta_changed kernel",
        ],
        "turn.ended 落了盘才起，两条都不带回合编号"
    );
    assert!(stage.log()[8..].iter().all(|event| event.cause.is_none()));
    assert_eq!(renamed(&stage), [kernel_titled("打个招呼")], "取第一行");
    let [called] = title_calls(&stage)[..] else {
        panic!("一次起标题");
    };
    assert_eq!(called.seen, seq(6), "名字是照到的那一条回答");
    assert_eq!(called.messages, 3, "替身的组装：两条清单加 title");
    assert_eq!(
        (called.first_difference.as_ref(), called.blocks.as_ref()),
        (None, None)
    );
    let [(upto, request)] = stage.titles() else {
        panic!("一次起标题的请求");
    };
    assert_eq!(*upto, seq(6));
    assert_eq!(
        listed_request(request),
        "2 message.user\n6 message.assistant\ntitle\n"
    );
    // 起过了就不再起；主请求没多。
    one_turn(&mut stage, "again");
    assert_eq!(stage.titles().len(), 1);
    assert_eq!(stage.requests().len(), 2);
    // 标题记在会话上：人再改成一样的，什么都不记。
    let same = stage.set_meta(Some("打个招呼"), None);
    assert_eq!(
        stage.outcome(&same),
        Some(&Outcome::Accepted { events: Vec::new() })
    );
}

#[test]
fn a_long_title_is_cut_at_the_limit_without_an_ellipsis() {
    let mut stage = titled_stage();
    let long = format!("{} {}", "字".repeat(49), "尾巴");
    stage.title_model([Line::says(&long)]);
    one_turn(&mut stage, "hi");
    assert_eq!(
        renamed(&stage),
        [kernel_titled(&"字".repeat(49))],
        "按字数截到 50 个，截完末尾的空白去掉"
    );
    let mut stage = titled_stage();
    stage.title_model([Line::says(&"題".repeat(60))]);
    one_turn(&mut stage, "hi");
    assert_eq!(renamed(&stage), [kernel_titled(&"題".repeat(50))]);
}

#[test]
fn a_session_named_by_a_person_is_left_alone() {
    let mut stage = titled_stage();
    stage.set_meta(Some("我起的"), None);
    one_turn(&mut stage, "hi");
    assert!(stage.titles().is_empty(), "起过名的不起");
    // 起过又去掉的也算人动过。
    let mut stage = titled_stage();
    stage.set_meta(Some("我起的"), None);
    stage.set_meta(Some(""), None);
    one_turn(&mut stage, "hi");
    assert!(stage.titles().is_empty(), "去掉过的也不起");
    // 只置顶的不算起名。
    let mut stage = titled_stage();
    stage.set_meta(None, Some(true));
    stage.title_model([Line::says("打招呼")]);
    one_turn(&mut stage, "hi");
    assert_eq!(stage.titles().len(), 1, "只置顶的照样起");
}

#[test]
fn children_and_one_shot_sessions_get_no_title() {
    let mut child = Stage::child(titled, environment("~/src/miyu"), at(0), PARENT);
    one_turn(&mut child, "读 Cargo.toml");
    assert!(child.titles().is_empty(), "子会话不起");
    let mut oneshot = Stage::oneshot(titled, environment("~/src/miyu"), at(0));
    one_turn(&mut oneshot, "hi");
    assert!(oneshot.titles().is_empty(), "一次性的会话不起");
}

#[test]
fn a_turn_without_an_answer_asks_for_nothing() {
    let mut stage = titled_stage();
    stage.model([Line::fails(ErrorClass::Auth, "401")]);
    stage.say("hi");
    assert!(stage.titles().is_empty(), "没答出正文的那一轮不起");
    stage.title_model([Line::says("打招呼")]);
    one_turn(&mut stage, "hi again");
    let [(upto, _)] = stage.titles() else {
        panic!("答出来的那一轮起");
    };
    assert_eq!(*upto, seq(10));
}

/// 起过一次没起成，下一轮只调了工具、没答出正文就出了错：这一轮不起，再下一轮答了才起。
#[test]
fn a_later_turn_without_an_answer_does_not_ask_again() {
    let mut stage = titled_stage();
    stage.title_model([Line::fails(ErrorClass::Retryable, "503")]);
    one_turn(&mut stage, "hi");
    stage.model([
        Line::calls("", &[("read", r#"{"path":"a.txt"}"#)]),
        Line::fails(ErrorClass::Auth, "401"),
    ]);
    stage.tools([Play::done("a")]);
    stage.say("读一下");
    assert_eq!(stage.titles().len(), 1, "这一轮没答出正文");
    stage.title_model([Line::says("打招呼")]);
    one_turn(&mut stage, "again");
    assert_eq!(stage.titles().len(), 2);
    assert_eq!(renamed(&stage), [kernel_titled("打招呼")]);
}

#[test]
fn it_tries_twice_then_gives_up() {
    let mut stage = titled_stage();
    stage.title_model([
        Line::fails(ErrorClass::Retryable, "503"),
        Line::says("  ").thinking("只想不说"),
    ]);
    one_turn(&mut stage, "hi");
    // 中间要一句回顾：回顾的请求不算起标题试过。
    stage.recap_model([Line::says("在打招呼。")]);
    stage.recap();
    one_turn(&mut stage, "again");
    one_turn(&mut stage, "third");
    assert_eq!(stage.titles().len(), 2, "第三轮不再试");
    let errors: Vec<(ErrorClass, &str)> = title_calls(&stage)
        .iter()
        .map(|called| {
            let error = called.error.as_ref().expect("没起成");
            (error.class.clone(), error.message.as_str())
        })
        .collect();
    assert_eq!(
        errors,
        [
            (ErrorClass::Retryable, "503"),
            (ErrorClass::EmptyReply, "the title reply has no text"),
        ]
    );
    assert!(renamed(&stage).is_empty());
}

#[test]
fn the_tries_are_counted_from_the_log_and_a_lost_one_does_not_count() {
    let mut stage = titled_stage();
    // 第一次没排剧本：一直在路上，跟着重启丢了，不算。
    one_turn(&mut stage, "hi");
    assert_eq!(stage.titles().len(), 1);
    stage.restart();
    stage.title_model([Line::fails(ErrorClass::Retryable, "503")]);
    one_turn(&mut stage, "again");
    assert_eq!(stage.titles().len(), 2, "丢了的不算，照样再试");
    stage.crash();
    stage.title_model([Line::fails(ErrorClass::Retryable, "503")]);
    one_turn(&mut stage, "third");
    one_turn(&mut stage, "fourth");
    assert_eq!(
        (stage.titles().len(), title_calls(&stage).len()),
        (3, 2),
        "载入以后照日志数：试过两次就停"
    );
}

#[test]
fn a_person_renaming_while_it_is_in_flight_wins() {
    let mut stage = titled_stage();
    stage.title_model([Line::says("自动的").held()]);
    one_turn(&mut stage, "hi");
    stage.set_meta(Some("人起的"), None);
    stage.release_title();
    assert_eq!(title_calls(&stage).len(), 1, "请求照记");
    assert_eq!(
        renamed(&stage),
        [(
            stage.log()[1].by.clone(),
            MetaChanged {
                title: Some("人起的".to_string()),
                pinned: None,
            }
        )],
        "不盖掉人起的"
    );
}

#[test]
fn one_at_a_time_and_undoing_the_turn_keeps_the_title() {
    let mut stage = titled_stage();
    stage.title_model([Line::says("打招呼").held()]);
    one_turn(&mut stage, "hi");
    one_turn(&mut stage, "again");
    assert_eq!(stage.titles().len(), 1, "在路上的时候不再起");
    // 第三轮正答着时回来：两条照样不带回合编号。
    stage.model([Line::says("嗯。").held()]);
    stage.say("third");
    stage.release_title();
    stage.release_model();
    assert_eq!(renamed(&stage), [kernel_titled("打招呼")]);
    let titled: Vec<&Event> = stage
        .log()
        .iter()
        .filter(|event| event.by == By::Kernel && event.turn.is_none())
        .filter(|event| matches!(event.body, Body::ModelCalled(_) | Body::MetaChanged(_)))
        .collect();
    assert_eq!(titled.len(), 2, "回合进行中回来的也不带回合编号");
    let first = stage.turns()[0];
    stage.revert(first);
    assert_eq!(
        renamed(&stage),
        [kernel_titled("打招呼")],
        "撤掉那一轮，标题照样在"
    );
    let same = stage.set_meta(Some("打招呼"), None);
    assert_eq!(
        stage.outcome(&same),
        Some(&Outcome::Accepted { events: Vec::new() })
    );
}

#[test]
fn nothing_is_asked_once_a_restart_is_coming() {
    let mut stage = titled_stage();
    stage.model([Line::calls("先看看。", &[("read", r#"{"path":"a.txt"}"#)])]);
    stage.tools([Play::done("a").held()]);
    stage.say("hi");
    stage.restarting();
    assert!(
        stage.log().iter().any(|event| matches!(&event.body, Body::TurnEnded(ended) if ended.reason == EndReason::Restarted)),
        "这一轮被重启打断了"
    );
    assert!(stage.titles().is_empty(), "要关了，不起");
}
