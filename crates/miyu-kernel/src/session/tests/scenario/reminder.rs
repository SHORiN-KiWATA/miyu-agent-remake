//! 场景：角色扮演提示（施工 P-1 补，`docs/blueprint/kernel/request.md`「事实」）。快照带着它的，第 1、4、7 轮开头注入一块，
//! 排在环境、权限后面；回合中途切了级别的边界不注入；载入以后照日志接着数；没有它的快照什么都不多。排在触发那句后面是
//! 组装的事，在 `miyu-assemble` 测。

use super::*;
use crate::event::Level;
use crate::facts::REMINDER;

/// 替身的策略加上角色扮演提示。
fn reminding() -> Policy {
    let mut policy = policy();
    policy.facts = policy.facts.with_reminder(Some("<r>stay</r>".to_string()));
    policy
}

fn stage() -> Stage {
    Stage::new(reminding, environment("~/src/miyu"), at(0))
}

/// 注入了提示的回合是第几个开过的（从 0 数），照先后。
fn reminded(stage: &Stage) -> Vec<usize> {
    let turns = stage.turns();
    stage
        .log()
        .iter()
        .filter_map(|event| match &event.body {
            Body::ContextInjected(fact) if fact.kind.as_str() == REMINDER => {
                assert_eq!(fact.text, "<r>stay</r>");
                let turn = event.turn.expect("事实带着回合");
                turns.iter().position(|opened| *opened == turn)
            }
            _ => None,
        })
        .collect()
}

#[test]
fn it_opens_the_first_turn_after_the_env_and_the_permission_and_every_third_after() {
    let mut stage = stage();
    stage.model((0..7).map(|_| Line::says("嗯。")));
    stage.say("hi");
    assert_eq!(
        story(&stage)[2..7],
        [
            "3 turn.started kernel t3",
            "4 context.injected:env kernel t3",
            "5 context.injected:permission kernel t3",
            "6 context.injected:reminder kernel t3",
            "7 message.assistant model t3",
        ]
    );
    for said in ["二", "三", "四", "五", "六", "七"] {
        stage.say(said);
    }
    assert_eq!(reminded(&stage), [0, 3, 6]);
}

#[test]
fn a_switch_mid_turn_does_not_bring_it() {
    let mut stage = stage();
    stage.model([
        Line::calls("我读一下。", &[("read", r#"{"path":"a"}"#)]),
        Line::says("读完了。"),
    ]);
    stage.tools([Play::done("A").held()]);
    stage.say("读 a");
    let reading = stage.ran()[0].0;
    stage.set_permission(Some(Level::Full), None);
    stage.release_tool(reading);
    assert_eq!(reminded(&stage), [0], "{:?}", story(&stage));
    let permissions = stage
        .log()
        .iter()
        .filter(|event| {
            matches!(&event.body, Body::ContextInjected(fact) if fact.kind.as_str() == "permission")
        })
        .count();
    assert_eq!(permissions, 2, "中途切的那一块照样有：{:?}", story(&stage));
}

#[test]
fn after_loading_the_count_goes_on_from_the_log() {
    let mut stage = stage();
    stage.model((0..4).map(|_| Line::says("嗯。")));
    stage.say("hi");
    stage.say("二");
    stage.crash();
    stage.say("三");
    stage.say("四");
    assert_eq!(reminded(&stage), [0, 3]);
}

#[test]
fn a_snapshot_without_it_injects_nothing_more() {
    let mut stage = Stage::new(policy, environment("~/src/miyu"), at(0));
    stage.model((0..4).map(|_| Line::says("嗯。")));
    for said in ["hi", "二", "三", "四"] {
        stage.say(said);
    }
    assert!(reminded(&stage).is_empty());
}
