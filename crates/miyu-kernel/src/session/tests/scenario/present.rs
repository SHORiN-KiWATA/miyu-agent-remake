//! 场景：压缩以后挂接点看到的 `present`（施工 R-4 上，`docs/blueprint/kernel/session.md` 的 `RunTurnStartHooks`）。模块
//! 交的一块在被压掉的那一段里，压完的下一轮不在，模块再交；在留下的那一截里（压的那一轮开头交的），压完照样在。

use super::compaction::{compacting, line};
use super::prepare::compactions;
use super::*;
use crate::event::ContextInjected;
use crate::facts::Present;
use crate::id::{FactKind, ModuleId};
use crate::session::Injection;

/// 记忆交的一块，带着 `m1`。
fn memory() -> Injection {
    Injection {
        module: ModuleId::parse("memory").unwrap(),
        fact: ContextInjected {
            kind: FactKind::parse("memory").unwrap(),
            text: "<memories>\nm1 user 2026-10-08: 喜欢喝茶\n</memories>\n".to_string(),
            refs: vec!["m1".to_string()],
        },
    }
}

/// 它在 `present` 里的样子。
fn present() -> Present {
    Present {
        module: ModuleId::parse("memory").unwrap(),
        kind: FactKind::parse("memory").unwrap(),
        refs: vec!["m1".to_string()],
    }
}

#[test]
fn a_block_in_the_compacted_part_is_gone_in_the_next_turn() {
    let mut stage = compacting(None);
    stage.hooks([vec![memory()]]);
    stage.model([Line::says("好。")]);
    stage.say("hi");
    line(&mut stage, 100);
    // 第二轮一开头就过线，压到第一轮的最后一条：第一轮开头交的那一块在被压掉的那一段里。
    stage.model([
        Line::says("S1"),
        Line::says("嗯。").reports(10),
        Line::says("在。"),
    ]);
    stage.say("再说一句");
    stage.say("还在吗");
    assert_eq!(story(&stage)[5], "6 context.injected:memory memory t3");
    assert_eq!(compactions(&stage)[0].upto, seq(9));
    let presents = stage.presents();
    assert_eq!(presents.len(), 3);
    assert!(presents[0].is_empty(), "第一轮还没有");
    assert_eq!(presents[1], [present()], "压之前，第二轮开头还在");
    assert!(presents[2].is_empty(), "压掉了：下一轮不在，模块再交");
}

#[test]
fn a_block_in_the_kept_part_stays() {
    let mut stage = compacting(None);
    stage.model([Line::says("好。")]);
    stage.say("hi");
    line(&mut stage, 100);
    // 第二轮开头交了那一块，接着过线、压到第一轮的最后一条：那一块在留下的那一截里。
    stage.hooks([vec![memory()]]);
    stage.model([
        Line::says("S1"),
        Line::says("嗯。").reports(10),
        Line::says("在。"),
    ]);
    stage.say("再说一句");
    stage.say("还在吗");
    assert_eq!(story(&stage)[10], "11 context.injected:memory memory t10");
    assert_eq!(compactions(&stage)[0].upto, seq(8));
    let presents = stage.presents();
    assert_eq!(presents.len(), 3);
    assert_eq!(presents[2], [present()], "留下的照样在");
}
