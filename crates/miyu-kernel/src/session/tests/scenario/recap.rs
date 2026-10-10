//! 场景：回顾（施工 3-8 四补，`docs/blueprint/kernel/session.md`「回顾」）。人要一句回顾：单独发一次请求，说完了记一条带
//! `purpose` 的 `model.called` 和 `session.recapped`，都不带回合编号，回应那一句；中间没有新内容的交回上一句、不请求；一个
//! 回复都没有的拒绝；有回合在进行时照收，不碰主请求（上一次请求、排着的话、压缩的边界）；在路上又来的并进去；没写成的拒绝、
//! 不再来；撤掉一轮以后照到的回到从前，交回那一句。
//!
//! 替身的组装一条事件一行（[`Listing`]）：回顾的请求是人的消息、回复的清单，最后一条写着「recap」。

use super::*;
use crate::event::{Purpose, SessionRecapped};
use crate::session::Compaction;

/// 第一轮说一句就完：日志到 8，6 是回复。
fn after_one_turn(stage: &mut Stage) {
    stage.model([Line::says("好。")]);
    stage.say("hi");
    assert_eq!(stage.log().len(), 8);
}

/// 第 `seq` 条。
fn event(stage: &Stage, seq: u64) -> &Event {
    &stage.log()[usize::try_from(seq - 1).unwrap()]
}

/// 日志里的回顾，照先后。
fn recapped(stage: &Stage) -> Vec<&SessionRecapped> {
    stage
        .log()
        .iter()
        .filter_map(|event| match &event.body {
            Body::SessionRecapped(recapped) => Some(recapped),
            _ => None,
        })
        .collect()
}

/// 回好了：那一句、照到的、是不是交回的。
fn recapped_as(text: &str, upto: u64, cached: bool) -> Outcome {
    Outcome::Recapped {
        text: text.to_string(),
        upto: seq(upto),
        cached,
    }
}

#[test]
fn a_recap_is_one_request_of_its_own_recorded_outside_any_turn() {
    let mut stage = stage();
    after_one_turn(&mut stage);
    stage.recap_model([Line::says("  在打招呼。\n").thinking("想想")]);
    let asked = stage.recap();
    assert_eq!(
        stage.outcome(&asked),
        Some(&recapped_as("在打招呼。", 6, false)),
        "回应那一句，去掉前后空白，思考不要"
    );
    assert_eq!(
        story(&stage)[8..],
        ["9 model.called:ok kernel", "10 session.recapped kernel"],
        "两条都不带回合编号，不开回合"
    );
    let called = &stage.model_calls()[1];
    assert_eq!(called.purpose, Some(Purpose::Recap));
    assert_eq!(called.seen, seq(6), "名字是照到的那一条");
    assert_eq!(called.messages, 3, "替身的组装：两条清单加 recap");
    assert_eq!(called.first_difference, None, "不和主请求比");
    assert_eq!(called.blocks, None, "不写回复");
    assert!(called.usage.is_some() && called.duration_ms.is_some());
    assert_eq!(event(&stage, 9).cause, Some(asked.clone()));
    assert_eq!(event(&stage, 10).cause, Some(asked));
    assert_eq!(
        recapped(&stage),
        [&SessionRecapped {
            text: "在打招呼。".to_string(),
            upto: seq(6),
        }]
    );
    assert_eq!(stage.requests().len(), 1, "主请求没多");
    let [(upto, request)] = stage.recaps() else {
        panic!("一次回顾的请求");
    };
    assert_eq!(*upto, seq(6));
    assert_eq!(
        listed_request(request),
        "2 message.user\n6 message.assistant\nrecap\n"
    );
}

#[test]
fn nothing_new_hands_back_the_last_one_without_asking() {
    let mut stage = stage();
    after_one_turn(&mut stage);
    stage.recap_model([Line::says("在打招呼。")]);
    stage.recap();
    let again = stage.recap();
    assert_eq!(
        stage.outcome(&again),
        Some(&recapped_as("在打招呼。", 6, true))
    );
    assert_eq!(stage.recaps().len(), 1, "没有再请求");
    assert_eq!(stage.log().len(), 10, "什么都没记");
    // 新说了一句，照到的变了，再请求一次。
    stage.model([Line::says("嗯。")]);
    stage.say("again");
    stage.recap_model([Line::says("又打了个招呼。")]);
    let third = stage.recap();
    assert_eq!(
        stage.outcome(&third),
        Some(&recapped_as("又打了个招呼。", 13, false))
    );
    assert_eq!(stage.recaps().len(), 2);
}

#[test]
fn without_a_reply_there_is_nothing_to_recap() {
    let mut stage = stage();
    let asked = stage.recap();
    assert_eq!(
        stage.outcome(&asked),
        Some(&Outcome::Rejected {
            reason: Reason::NothingToRecap,
            about: Vec::new(),
        })
    );
    stage.model([Line::fails(ErrorClass::Auth, "401")]);
    stage.say("hi");
    let again = stage.recap();
    assert_eq!(
        stage.outcome(&again),
        Some(&Outcome::Rejected {
            reason: Reason::NothingToRecap,
            about: Vec::new(),
        }),
        "说了一句、没回复的也没有"
    );
    assert!(stage.recaps().is_empty());
}

#[test]
fn a_failed_recap_is_refused_and_not_tried_again() {
    let mut stage = stage();
    after_one_turn(&mut stage);
    stage.recap_model([
        Line::fails(ErrorClass::Retryable, "503"),
        Line::says("").thinking("只想不说"),
        Line::calls("", &[("read", "{}")]),
    ]);
    for _ in 0..3 {
        let asked = stage.recap();
        assert_eq!(
            stage.outcome(&asked),
            Some(&Outcome::Rejected {
                reason: Reason::RecapFailed,
                about: Vec::new(),
            }),
            "出错、没有正文都没写成，不再来"
        );
    }
    assert_eq!(stage.recaps().len(), 3, "一次一次，不重试");
    assert!(recapped(&stage).is_empty());
    let classes: Vec<Option<ErrorClass>> = stage.model_calls()[1..]
        .iter()
        .map(|called| called.error.as_ref().map(|error| error.class.clone()))
        .collect();
    assert_eq!(
        classes,
        [
            Some(ErrorClass::Retryable),
            Some(ErrorClass::EmptyReply),
            Some(ErrorClass::EmptyReply)
        ]
    );
}

#[test]
fn asked_again_while_one_is_on_its_way_both_get_the_same() {
    let mut stage = stage();
    after_one_turn(&mut stage);
    stage.recap_model([Line::says("在打招呼。").held()]);
    let first = stage.recap();
    let second = stage.recap();
    assert_eq!(stage.outcome(&first), None, "还在路上");
    assert_eq!(stage.outcome(&second), None);
    stage.release_recap();
    let answer = recapped_as("在打招呼。", 6, false);
    assert_eq!(stage.outcome(&first), Some(&answer));
    assert_eq!(stage.outcome(&second), Some(&answer), "并进去的一起回");
    assert_eq!(stage.recaps().len(), 1, "只请求一次");
    assert_eq!(
        event(&stage, 10).cause,
        Some(first),
        "事件的 cause 是要它的那一个"
    );
}

/// 有回合在进行时照收：照这时的有效历史，不碰这一轮。这一轮接下来的请求照样只是往后长（不和回顾的请求比）。
#[test]
fn during_a_turn_it_is_taken_and_leaves_the_turn_alone() {
    let mut stage = stage();
    after_one_turn(&mut stage);
    stage.model([
        Line::calls("我读一下。", &[("read", r#"{"path":"a"}"#)]).held(),
        Line::says("读完了。"),
    ]);
    stage.tools([Play::done("A")]);
    stage.say("读 a");
    stage.recap_model([Line::says("在读 a。")]);
    let asked = stage.recap();
    assert_eq!(
        stage.outcome(&asked),
        Some(&recapped_as("在读 a。", 9, false)),
        "照到的是这一轮那句话"
    );
    assert_eq!(event(&stage, 11).turn, None);
    assert_eq!(event(&stage, 12).turn, None);
    stage.release_model();
    assert_eq!(
        story(&stage).last().map(String::as_str),
        Some("18 turn.ended:completed kernel t10"),
        "这一轮照常走完"
    );
    let main: Vec<_> = stage
        .model_calls()
        .into_iter()
        .filter(|called| !called.aside())
        .collect();
    assert_eq!(main.len(), 3);
    assert_eq!(
        main[2].first_difference, None,
        "回顾以后的主请求照样接着上一次主请求往后长"
    );
}

/// 有回合在进行时回顾，排着的那句她还没听到：撤回（打断退回）照样撤得了，账本不把回顾当成听到了。
#[test]
fn a_recap_does_not_count_as_hearing_what_is_queued() {
    let mut stage = stage();
    after_one_turn(&mut stage);
    stage.model([Line::says("好的。").held()]);
    stage.say("读 a");
    let queued = stage.say("顺便读 b");
    stage.recap_model([Line::says("在读 a、b。")]);
    stage.recap();
    assert_eq!(stage.recaps()[0].0, seq(11), "回顾照到了排着的那句");
    stage.interrupt(Queued::Return);
    let withdrawn = stage
        .log()
        .iter()
        .find_map(|event| match &event.body {
            Body::MessageWithdrawn(withdrawn) => Some(withdrawn.messages.clone()),
            _ => None,
        })
        .expect("撤回了排着的那句");
    assert_eq!(withdrawn, [seq(11)]);
    assert_eq!(
        stage.outcome(&queued),
        Some(&Outcome::Accepted {
            events: seqs(&[11])
        })
    );
}

/// 手动压缩的边界不把回顾当成请求：没人看着的一次性会话只记下的那句，回顾照到了它，它还是没听到的，留在检查点后面。
#[test]
fn compaction_keeps_what_only_a_recap_saw() {
    let make = || {
        let mut policy = policy();
        policy.compaction = Some(Compaction {
            reserve_cap: 10,
            margin: 10,
            line_percent: 100,
            margin_percent: 100,
            tail: 0,
            lead: 0,
            price: crate::estimate::Flat {
                image: 50,
                file: 50,
            },
            rebuild: None,
            pause: None,
            shorten: None,
            isolate: false,
        });
        policy
    };
    let mut stage = Stage::oneshot(make, environment("~/src/miyu"), at(0));
    stage.limits(Some(100_000), None);
    after_one_turn(&mut stage);
    stage.harness_says("claude-code", "CI 修好了。");
    assert_eq!(stage.turns().len(), 1, "没人看着：只记下");
    stage.recap_model([Line::says("CI 修好了。")]);
    stage.recap();
    assert_eq!(stage.recaps()[0].0, seq(9), "回顾照到了那句");
    stage.model([Line::says("S1")]);
    stage.request_compaction(None);
    let compacted = stage
        .log()
        .iter()
        .find_map(|event| match &event.body {
            Body::ContextCompacted(compacted) => Some(compacted.upto),
            _ => None,
        })
        .expect("压了");
    assert_eq!(compacted, seq(8), "那句她没听到，不压进摘要");
}

/// 撤掉后来的一轮，照到的回到撤之前：回顾的两条不带回合编号，撤哪一轮都不会跟着拿走；最近一条回顾照到的不是现在的，
/// 照撤完的有效历史再请求一次。
#[test]
fn after_an_undo_it_goes_by_what_is_left() {
    let mut stage = stage();
    after_one_turn(&mut stage);
    stage.recap_model([
        Line::says("在打招呼。"),
        Line::says("又打了个招呼。"),
        Line::says("还在打招呼。"),
    ]);
    stage.recap();
    stage.model([Line::says("嗯。")]);
    stage.say("again");
    stage.recap();
    stage.revert(TurnId::new(seq(12)));
    let asked = stage.recap();
    assert_eq!(
        stage.outcome(&asked),
        Some(&recapped_as("还在打招呼。", 6, false))
    );
    let (_, request) = stage.recaps().last().unwrap();
    assert_eq!(
        listed_request(request),
        "2 message.user\n6 message.assistant\nrecap\n",
        "撤掉的那一轮不在里面"
    );
    assert_eq!(recapped(&stage).len(), 3, "前两句都还在日志里");
}
