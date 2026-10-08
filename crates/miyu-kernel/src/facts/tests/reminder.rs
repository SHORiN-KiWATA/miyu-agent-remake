//! 角色扮演提示（施工 P-1 补，`docs/blueprint/kernel/request.md`「事实」）：没有原文的不注入；有的第一轮注入，之后数
//! 它后面开了几轮，连这一轮到 3 再注入，所以是第 1、4、7 轮；压缩替掉、撤掉了带着它的那一轮，下一轮重来；模块注入的
//! 同类块不算。

use super::*;

/// 带着角色扮演提示的模板。
fn reminding() -> FactTemplates {
    templates().with_reminder(Some("<r>stay</r>".to_string()))
}

/// 开一轮，照该不该注入的注入，结束。返回这一轮注入了没有。
fn turn(log: &mut Log, templates: &FactTemplates) -> bool {
    log.start();
    let due = templates.reminder(&log.history);
    if let Some(fact) = &due {
        log.inject(KERNEL, fact.kind.as_str(), &fact.text);
    }
    log.end();
    due.is_some()
}

#[test]
fn without_a_reminder_nothing_is_injected() {
    let mut log = Log::new();
    let plain = templates();
    assert!(!(0..5).any(|_| turn(&mut log, &plain)));
    let empty = templates().with_reminder(None);
    assert_eq!(empty, templates(), "没有的和以前一样");
}

#[test]
fn the_reminder_is_its_own_kind_and_text() {
    let mut log = Log::new();
    log.start();
    assert_eq!(
        reminding().reminder(&log.history),
        Some(fact(REMINDER, "<r>stay</r>"))
    );
}

#[test]
fn it_comes_on_the_first_turn_and_every_third_after() {
    let mut log = Log::new();
    let templates = reminding();
    let seen: Vec<bool> = (0..8).map(|_| turn(&mut log, &templates)).collect();
    assert_eq!(
        seen,
        [true, false, false, true, false, false, true, false],
        "第 1、4、7 轮"
    );
}

#[test]
fn a_compaction_that_replaced_it_brings_it_back_next_turn() {
    let mut log = Log::new();
    let templates = reminding();
    assert!(turn(&mut log, &templates));
    log.start();
    log.compact();
    log.end();
    assert!(turn(&mut log, &templates), "压缩替掉了，下一轮重来");
    assert!(!turn(&mut log, &templates));
}

#[test]
fn undoing_the_turn_that_had_it_brings_it_back_next_turn() {
    let mut log = Log::new();
    let templates = reminding();
    let first = log.ledger.next_seq().get() + 1;
    assert!(turn(&mut log, &templates));
    let second = log.ledger.next_seq().get() + 1;
    assert!(!turn(&mut log, &templates));
    // 撤一轮要连它后面的一起撤（账本）。
    log.push(
        ALICE,
        "turn.reverted",
        &format!(r#"{{"turns":[{first},{second}]}}"#),
    );
    assert!(turn(&mut log, &templates), "撤掉了带着它的那一轮");
}

#[test]
fn a_module_block_of_the_same_kind_does_not_count() {
    let mut log = Log::new();
    log.start();
    log.inject(MEMORY, REMINDER, "<r>stay</r>");
    assert!(reminding().reminder(&log.history).is_some());
}
