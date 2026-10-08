//! 场景：人切了权限级别，她看到的那一块（施工 2-7 补，`docs/blueprint/kernel/request.md`「事实」第 2 条）。第一轮写平常
//! 那一份；空闲时切的，下一轮开头用切换那一份、带上一块的级别；回合中途切的，这一轮下一次请求之前就用它；在一个边界之前
//! 切过去又切回来的，不注入；压缩以后、撤掉了带着上一块的几轮以后，照有效历史里还剩的算；撤掉的回合里切的照样算，载入
//! 以后也是；载入以后不重发；以前造的快照没有这份模板，照旧写平常那一份。

use super::*;
use crate::event::Level;
use crate::session::Compaction;

/// 替身的策略加上切换那一份模板：短，一眼认得出是哪个字段。
fn with_switch() -> Policy {
    let mut policy = policy();
    policy.facts = FactTemplates::new(
        r#"<e t="{time}" z="{timezone}" d="{cwd}"/>"#,
        r#"<p l="{level}"/>"#,
        "<reply-cut/>",
        None,
        Some(r#"<p l="{level}" was="{previous}"/>"#),
    )
    .unwrap();
    policy
}

/// 一个带切换模板的替身。
fn switching() -> Stage {
    Stage::new(with_switch, environment("~/src/miyu"), at(0))
}

/// 日志里内核注入的权限那几块：原文和它在第几个开过的回合（从 0 数），照先后。
fn told(stage: &Stage) -> Vec<(String, usize)> {
    let turns = stage.turns();
    stage
        .log()
        .iter()
        .filter_map(|event| match &event.body {
            Body::ContextInjected(fact) if fact.kind.as_str() == "permission" => {
                let turn = event.turn.expect("事实带着回合");
                let index = turns.iter().position(|opened| *opened == turn);
                Some((fact.text.clone(), index.expect("是开过的回合")))
            }
            _ => None,
        })
        .collect()
}

/// 平常那一份写出的 `level`，在第 `turn` 个回合。
fn plain(level: &str, turn: usize) -> (String, usize) {
    (format!(r#"<p l="{level}"/>"#), turn)
}

/// 切换那一份写出的从 `previous` 切到 `level`，在第 `turn` 个回合。
fn switched(level: &str, previous: &str, turn: usize) -> (String, usize) {
    (format!(r#"<p l="{level}" was="{previous}"/>"#), turn)
}

/// 切到完全放开。
fn to_full(stage: &mut Stage) {
    stage.set_permission(Some(Level::Full), None);
}

#[test]
fn a_switch_between_turns_is_told_with_the_level_before() {
    let mut stage = switching();
    stage.model([
        Line::says("好。"),
        Line::says("能写了。"),
        Line::says("嗯。"),
    ]);
    stage.say("hi");
    to_full(&mut stage);
    stage.say("现在呢");
    stage.say("再说一句");
    assert_eq!(
        told(&stage),
        [plain("workspace", 0), switched("full", "workspace", 1)]
    );
    // 切换那一块排在第二轮开头，就在回合开始的那一条后面；第三轮不再注入。
    assert_eq!(
        story(&stage)[8..13],
        [
            "9 session.policy_changed alice",
            "10 message.user alice",
            "11 turn.started kernel t11",
            "12 context.injected:permission kernel t11",
            "13 message.assistant model t11",
        ]
    );
}

#[test]
fn switching_there_and_back_before_a_boundary_tells_nothing() {
    let mut stage = switching();
    stage.model([
        Line::says("好。"),
        Line::calls("我读一下。", &[("read", r#"{"path":"a"}"#)]),
        Line::says("读完了。"),
    ]);
    stage.tools([Play::done("A").held()]);
    stage.say("hi");
    // 空闲时切过去又切回来。
    to_full(&mut stage);
    stage.set_permission(Some(Level::Workspace), None);
    stage.say("读 a");
    // 回合中途，工具在跑时开了只读又关掉。
    let reading = stage.ran()[0].0;
    stage.set_permission(None, Some(true));
    stage.set_permission(None, Some(false));
    stage.release_tool(reading);
    assert_eq!(told(&stage), [plain("workspace", 0)]);
    assert_eq!(stage.requests().len(), 3, "第二轮的两次请求都发了");
}

#[test]
fn a_switch_mid_turn_is_told_before_the_next_request_of_that_turn() {
    let mut stage = switching();
    stage.model([
        Line::calls("我读一下。", &[("read", r#"{"path":"a"}"#)]),
        Line::says("读完了。"),
    ]);
    stage.tools([Play::done("A").held()]);
    stage.say("读 a");
    let reading = stage.ran()[0].0;
    to_full(&mut stage);
    stage.release_tool(reading);
    assert_eq!(
        told(&stage),
        [plain("workspace", 0), switched("full", "workspace", 0)]
    );
    // 排在工具结果后面，第二次请求看得到它。
    let log = stage.log();
    let block = log
        .iter()
        .rev()
        .find(|event| matches!(event.body, Body::ContextInjected(_)))
        .expect("有切换那一块");
    let result = log
        .iter()
        .find(|event| matches!(event.body, Body::ToolResult(_)))
        .expect("有工具结果");
    assert!(block.seq > result.seq, "{:?}", story(&stage));
    assert!(stage.requests()[1].0 >= block.seq, "{:?}", story(&stage));
}

#[test]
fn a_chain_of_switches_names_the_level_she_saw_last() {
    let mut stage = switching();
    stage.model([Line::says("好。"), Line::says("嗯。"), Line::says("在。")]);
    stage.say("hi");
    to_full(&mut stage);
    stage.say("现在呢");
    stage.set_permission(None, Some(true));
    stage.say("那现在呢");
    assert_eq!(
        told(&stage),
        [
            plain("workspace", 0),
            switched("full", "workspace", 1),
            switched("read_only", "full", 2),
        ]
    );
}

#[test]
fn after_a_compaction_the_level_is_told_plainly() {
    let make = || {
        let mut policy = with_switch();
        policy.compaction = Some(Compaction {
            reserve_cap: 10,
            margin: 10,
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
    let mut stage = Stage::new(make, environment("~/src/miyu"), at(0));
    stage.model([Line::says("好。"), Line::says("能写了。")]);
    stage.say("hi");
    to_full(&mut stage);
    stage.say("现在呢");
    // 第三轮开头过线：前两轮连同两块权限压进摘要，压完她看不到上一块了。
    super::compaction::line(&mut stage, 100);
    stage.model([Line::says("S1"), Line::says("嗯。")]);
    stage.say("再说一句");
    assert_eq!(
        told(&stage),
        [
            plain("workspace", 0),
            switched("full", "workspace", 1),
            plain("full", 2),
        ],
        "{:?}",
        story(&stage)
    );
}

#[test]
fn after_undoing_the_turns_that_told_it_the_level_she_still_sees_counts() {
    let mut stage = switching();
    stage.model([
        Line::says("好。"),
        Line::says("能写了。"),
        Line::says("只读了。"),
        Line::says("嗯。"),
    ]);
    stage.say("hi");
    to_full(&mut stage);
    stage.say("现在呢");
    stage.set_permission(None, Some(true));
    let undone = stage.revert(stage.turns()[1]);
    assert!(matches!(
        stage.outcome(&undone),
        Some(Outcome::Accepted { .. })
    ));
    stage.say("还在吗");
    assert_eq!(
        told(&stage),
        [
            plain("workspace", 0),
            switched("full", "workspace", 1),
            switched("read_only", "workspace", 2),
        ],
        "撤掉了切到完全放开的那一轮，她最近看到的是工作区"
    );
}

#[test]
fn after_undoing_every_turn_that_told_a_level_it_is_told_plainly() {
    let mut stage = switching();
    stage.model([Line::says("好。"), Line::says("嗯。")]);
    stage.say("hi");
    to_full(&mut stage);
    let undone = stage.revert(stage.turns()[0]);
    assert!(matches!(
        stage.outcome(&undone),
        Some(Outcome::Accepted { .. })
    ));
    stage.say("还在吗");
    assert_eq!(told(&stage), [plain("workspace", 0), plain("full", 1)]);
}

#[test]
fn a_switch_in_an_undone_turn_still_counts_after_loading() {
    let mut stage = switching();
    stage.model([
        Line::says("好。"),
        Line::calls("我读一下。", &[("read", r#"{"path":"a"}"#)]),
        Line::says("读完了。"),
        Line::says("在。"),
    ]);
    stage.tools([Play::done("A").held()]);
    stage.say("hi");
    stage.say("读 a");
    let reading = stage.ran()[0].0;
    to_full(&mut stage);
    stage.release_tool(reading);
    let undone = stage.revert(stage.turns()[1]);
    assert!(matches!(
        stage.outcome(&undone),
        Some(Outcome::Accepted { .. })
    ));
    // 撤掉的那一轮里切的照样算：载入以后现在的权限还是完全放开，她看到的只剩工作区那一块。
    stage.crash();
    stage.say("还在吗");
    assert_eq!(
        told(&stage),
        [
            plain("workspace", 0),
            switched("full", "workspace", 1),
            switched("full", "workspace", 2),
        ]
    );
}

#[test]
fn after_loading_the_switch_is_not_told_again() {
    let mut stage = switching();
    stage.model([
        Line::says("好。"),
        Line::says("能写了。"),
        Line::says("嗯。"),
    ]);
    stage.say("hi");
    to_full(&mut stage);
    stage.say("现在呢");
    stage.crash();
    stage.say("再说一句");
    assert_eq!(
        told(&stage),
        [plain("workspace", 0), switched("full", "workspace", 1)]
    );
}

#[test]
fn older_snapshots_without_the_template_tell_the_plain_block() {
    let mut stage = Stage::new(policy, environment("~/src/miyu"), at(0));
    stage.model([Line::says("好。"), Line::says("能写了。")]);
    stage.say("hi");
    to_full(&mut stage);
    stage.say("现在呢");
    assert_eq!(told(&stage), [plain("workspace", 0), plain("full", 1)]);
}
