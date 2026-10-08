//! 场景：换策略快照（施工 P-1 再补，`docs/blueprint/kernel/session.md`「换策略快照」）。执行器在回合开始时交来新的一份：
//! 内核记一条带 `policy` 的 `session.policy_changed`（内核记的，排在注入前面），这一轮第一次请求就照新的拼；撤掉那一轮
//! 不换回去；挂接点跑完时没带哈希的，放着的扔掉。

use super::*;
use crate::assemble::Assembler;
use crate::event::PolicyChanged;
use crate::history::History;
use crate::id::ContentHash;

/// 新快照的哈希。
const NEW: &str = "sha256:5ed1f2a0c3b4d5e6f708192a3b4c5d6e7f8091a2b3c4d5e6f708192a3b4c5d6e";

/// 换上的那一份组装：照替身的拼，system 写 `swapped`。
struct Swapped;

impl Assembler for Swapped {
    fn assemble(&self, history: &History) -> Request {
        let mut request = Listing.assemble(history);
        request.system = "swapped".to_string();
        request
    }

    fn summarize(
        &self,
        history: &History,
        upto: Seq,
        cut: Option<Seq>,
        instructions: Option<&str>,
    ) -> Request {
        Listing.summarize(history, upto, cut, instructions)
    }

    fn summarize_isolated(
        &self,
        history: &History,
        upto: Seq,
        cut: Option<Seq>,
        instructions: Option<&str>,
    ) -> Request {
        Listing.summarize_isolated(history, upto, cut, instructions)
    }

    fn summary(&self, reply: &[Block]) -> Option<String> {
        Listing.summary(reply)
    }
}

/// 换上的那一份策略：组装换成 [`Swapped`]，别的和替身的一样。
fn swapped() -> Policy {
    let mut policy = policy();
    policy.assembler = Box::new(Swapped);
    policy
}

/// 每一次请求的 system，照先后。
fn systems(stage: &Stage) -> Vec<String> {
    stage
        .requests()
        .iter()
        .map(|(_, request)| request.system.clone())
        .collect()
}

#[test]
fn the_turn_after_a_swap_asks_with_the_new_policy() {
    let mut stage = Stage::new(policy, environment("~/src/miyu"), at(0));
    stage.model([Line::says("好。"), Line::says("嗯。"), Line::says("在。")]);
    stage.say("hi");
    stage.swap_policy(NEW, swapped);
    stage.say("再说一句");
    assert_eq!(
        story(&stage)[8..12],
        [
            "9 message.user alice",
            "10 turn.started kernel t10",
            "11 session.policy_changed kernel t10",
            "12 message.assistant model t10",
        ]
    );
    let changed = &stage.log()[10];
    assert_eq!(
        changed.body,
        Body::PolicyChanged(PolicyChanged {
            policy: Some(ContentHash::parse(NEW).unwrap()),
            ..PolicyChanged::default()
        })
    );
    assert_eq!(changed.cause, stage.log()[9].cause, "cause 是这一轮的");
    stage.say("还在吗");
    assert_eq!(systems(&stage), ["listing", "swapped", "swapped"]);
}

#[test]
fn undoing_the_turn_that_swapped_does_not_swap_back() {
    let mut stage = Stage::new(policy, environment("~/src/miyu"), at(0));
    stage.model([Line::says("好。"), Line::says("嗯。"), Line::says("在。")]);
    stage.say("hi");
    stage.swap_policy(NEW, swapped);
    stage.say("再说一句");
    let undone = stage.revert(TurnId::new(seq(10)));
    assert!(matches!(
        stage.outcome(&undone),
        Some(Outcome::Accepted { .. })
    ));
    stage.say("还在吗");
    assert_eq!(systems(&stage), ["listing", "swapped", "swapped"]);
    let swaps = stage
        .log()
        .iter()
        .filter(
            |event| matches!(&event.body, Body::PolicyChanged(changed) if changed.policy.is_some()),
        )
        .count();
    assert_eq!(swaps, 1, "不重记");
}

#[test]
fn a_staged_policy_without_a_hash_is_dropped() {
    let mut session = session();
    session.stage_policy(swapped());
    session.handle(send(2, "hi"));
    session.handle(stored(5));
    // 挂接点跑完时没带哈希：不记、不换，放着的扔掉。
    let actions = session.handle(Input::TurnStartHooksDone {
        at: at(30),
        turn: TurnId::new(seq(3)),
        injected: Vec::new(),
        replaced: None,
        policy: None,
        prepare: false,
    });
    assert!(appended_events(&actions).is_empty(), "{actions:?}");
    let systems: Vec<&str> = actions
        .iter()
        .filter_map(|action| match action {
            Action::CallModel { request, .. } => Some(request.system.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(systems, ["listing"]);
}

/// 换上的那一份带角色扮演提示、旧的没有：回合开头照旧的查过，换上以后照新的再查一次，这一轮就带上（排在换快照那一条后面）。
#[test]
fn a_reminder_of_the_new_policy_comes_in_the_turn_it_swaps() {
    let mut stage = Stage::new(policy, environment("~/src/miyu"), at(0));
    stage.model([Line::says("好。"), Line::says("嗯。")]);
    stage.say("hi");
    stage.swap_policy(NEW, || {
        let mut policy = swapped();
        policy.facts = policy.facts.with_reminder(Some("<r/>".to_string()));
        policy
    });
    stage.say("再说一句");
    assert_eq!(
        story(&stage)[9..12],
        [
            "10 turn.started kernel t10",
            "11 session.policy_changed kernel t10",
            "12 context.injected:reminder kernel t10",
        ]
    );
}
