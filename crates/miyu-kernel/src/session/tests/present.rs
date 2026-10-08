//! 回合开始交给挂接点的「有效历史里模块注入过的那几块」（施工 R-4 上，`docs/blueprint/kernel/session.md` 的
//! `RunTurnStartHooks`）：模块交的一块下一轮在 `present` 里，`refs` 原样记、原样交回；清空、撤掉带着它的那一轮以后不在了；
//! 内核自己注入的不算；没有模块注入的会话 `present` 是空的。

use super::clear::clear;
use super::executor::injection;
use super::load::Logged;
use super::*;
use crate::facts::Present;
use crate::id::{FactKind, ModuleId};

/// 记忆这个模块交的一块，带着这几条的编号。
fn memory_block(refs: &[&str]) -> Injection {
    let mut block = injection(
        "memory",
        "<memories>\nm1 user 2026-09-25: 喜欢喝茶\n</memories>\n",
    );
    block.fact.refs = refs.iter().map(ToString::to_string).collect();
    block
}

/// 发一句，开回合，开头落了盘：交回这个回合和内核叫挂接点时带的 `present`。
fn hooked(logged: &mut Logged, n: u64, words: &str) -> (TurnId, Vec<Present>) {
    logged.handle(send(n, words));
    let actions = logged.handle(stored(logged.last()));
    actions
        .iter()
        .find_map(|action| match action {
            Action::RunTurnStartHooks { turn, present, .. } => Some((*turn, present.clone())),
            _ => None,
        })
        .expect("叫了挂接点")
}

/// 挂接点交回 `injected`，落了盘，模型说了一句，落了盘：这一轮说完。
fn finish(logged: &mut Logged, turn: TurnId, injected: Vec<Injection>) {
    let mut actions = logged.handle(hooks_done(turn, injected));
    if calls(&actions).is_empty() {
        actions = logged.handle(stored(logged.last()));
    }
    let (seen, _) = calls(&actions).remove(0);
    logged.say(seen.get(), "好。");
}

fn memory() -> Present {
    Present {
        module: ModuleId::parse("memory").unwrap(),
        kind: FactKind::parse("memory").unwrap(),
        refs: vec!["m1".to_string(), "m2".to_string()],
    }
}

#[test]
fn a_module_block_is_present_in_the_next_turn_with_its_refs() {
    let mut logged = Logged::new();
    let (turn, present) = hooked(&mut logged, 2, "hi");
    assert!(
        present.is_empty(),
        "还没有模块注入过；内核自己的两块不算：{present:?}"
    );
    finish(&mut logged, turn, vec![memory_block(&["m1", "m2"])]);
    let recorded = logged
        .log
        .iter()
        .find_map(|event| match &event.body {
            Body::ContextInjected(fact) if fact.kind.as_str() == "memory" => Some(fact.clone()),
            _ => None,
        })
        .expect("记进了日志");
    assert_eq!(recorded.refs, ["m1", "m2"], "refs 原样记");

    let (turn, present) = hooked(&mut logged, 3, "再说一句");
    assert_eq!(present, [memory()], "下一轮交给挂接点，refs 原样交回");
    finish(&mut logged, turn, Vec::new());
    let (_, present) = hooked(&mut logged, 4, "第三句");
    assert_eq!(present, [memory()], "还在：一个会话一份，会话中途不重发");
}

#[test]
fn clearing_and_undoing_its_turn_take_the_block_away() {
    let mut logged = Logged::new();
    let (turn, _) = hooked(&mut logged, 2, "hi");
    finish(&mut logged, turn, vec![memory_block(&["m1", "m2"])]);
    logged.handle(clear(3));
    logged.handle(stored(logged.last()));
    let (turn, present) = hooked(&mut logged, 4, "清空以后");
    assert!(present.is_empty(), "检查点以前的不算：{present:?}");

    finish(&mut logged, turn, vec![memory_block(&["m1", "m2"])]);
    let (turn, present) = hooked(&mut logged, 5, "又一句");
    assert_eq!(present, [memory()]);
    finish(&mut logged, turn, Vec::new());
    // 撤掉带着那一块的那一轮（清空以后的第一轮）：撤销照回合算，那一轮以后的都撤掉。
    let with_block = logged
        .log
        .iter()
        .rev()
        .find_map(|event| match &event.body {
            Body::ContextInjected(fact) if fact.kind.as_str() == "memory" => event.turn,
            _ => None,
        })
        .expect("有那一块");
    let undo = Input::Command(Received {
        id: id(6),
        by: alice(),
        at: at(6),
        command: Command::Revert {
            turn: Some(with_block),
        },
    });
    logged.handle(undo);
    logged.handle(stored(logged.last()));
    let (_, present) = hooked(&mut logged, 7, "撤销以后");
    assert!(present.is_empty(), "撤掉的回合里的不算：{present:?}");
}

#[test]
fn an_old_block_without_refs_reads_and_writes_the_same() {
    let old = r#"{"kind":"memory","text":"<memories/>\n"}"#;
    let fact: ContextInjected = serde_json::from_str(old).unwrap();
    assert!(fact.refs.is_empty());
    assert_eq!(serde_json::to_string(&fact).unwrap(), old, "没有的不写");
    let with = r#"{"kind":"memory","text":"<memories/>\n","refs":["m3"]}"#;
    let fact: ContextInjected = serde_json::from_str(with).unwrap();
    assert_eq!(fact.refs, ["m3"]);
    assert_eq!(serde_json::to_string(&fact).unwrap(), with);
}
