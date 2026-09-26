//! 样本会话里该注入的事实（`docs/designs/08-上下文投影.md` 第五节「环境和状态的事实怎么写」）：
//! 用出厂的两个模板，照样本会话当时的环境和权限，算出每个边界上内核该注入的几块。
//!
//! - 42 号回合开始时：两块都要注入，就是样本里的 43、44 号；
//! - 51 号换成只读以后：下一个边界只注入权限那一块；
//! - 52 号撤销了那一轮以后：两块都要重新注入；
//! - 53 号压缩以后：也是两块都要重新注入。
//!
//! 出厂的模板在编译时拿进来；样本要读文件，纯逻辑门禁只扫 `src/`，集成测试可以读。样本的序号
//! 中间有空当（省掉了前面的几十条），过不了账本，所以直接交给有效历史。

use std::fs;
use std::path::PathBuf;

use miyu_kernel::event::{Body, ContextInjected, Event, Permission};
use miyu_kernel::facts::{Environment, FactTemplates, changed};
use miyu_kernel::history::History;
use miyu_kernel::origin::By;
use miyu_kernel::time::{Timestamp, UtcOffset};

/// 出厂的两个模板。
fn templates() -> FactTemplates {
    FactTemplates::new(
        include_str!("../../../resources/core/facts/env.txt"),
        include_str!("../../../resources/core/facts/permission.txt"),
    )
    .expect("出厂的模板用得了")
}

/// 样本会话的全部事件，照序号排好。一份样本可以有几行。
fn events() -> Vec<Event> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/designs/samples/events");
    let entries =
        fs::read_dir(&dir).unwrap_or_else(|e| panic!("读不了样本目录 {}：{e}", dir.display()));
    let mut events = Vec::new();
    for entry in entries {
        let path = entry.expect("列样本目录时出错").path();
        let text =
            fs::read_to_string(&path).unwrap_or_else(|e| panic!("读不了 {}：{e}", path.display()));
        for line in text.lines() {
            let event = Event::from_line(line)
                .unwrap_or_else(|e| panic!("{} 读不出来：{e}", path.display()));
            events.push(event);
        }
    }
    events.sort_by_key(|event| event.seq);
    events
}

/// 序号不超过 `upto` 的事件。
fn upto(events: &[Event], upto: u64) -> impl Iterator<Item = &Event> {
    events.iter().filter(move |event| event.seq.get() <= upto)
}

/// 到第 `seq` 条为止，会话的权限：创建时定的，被后来换过的盖掉。
fn permission_at(events: &[Event], seq: u64) -> Permission {
    let mut permission = None;
    for event in upto(events, seq) {
        match &event.body {
            Body::SessionCreated(created) => permission = Some(created.permission.clone()),
            Body::PolicyChanged(changed) => {
                if let Some(new) = &changed.permission {
                    permission = Some(new.clone());
                }
            }
            _ => {}
        }
    }
    permission.expect("样本会话有 session.created")
}

/// 样本会话的环境：东九区，工作目录 `~/src/miyu`。
fn environment() -> Environment {
    Environment {
        offset: UtcOffset::from_minutes(540).expect("东九区在范围里"),
        cwd: "~/src/miyu".to_string(),
    }
}

/// 第 `seq` 条的时刻。
fn time_of(events: &[Event], seq: u64) -> Timestamp {
    events
        .iter()
        .find(|event| event.seq.get() == seq)
        .unwrap_or_else(|| panic!("样本会话里没有 {seq} 号"))
        .at
}

/// 第 `seq` 条之后的边界上，内核该注入的几块。
fn injected_after(events: &[Event], seq: u64) -> Vec<ContextInjected> {
    let mut history = History::default();
    for event in upto(events, seq) {
        history.append(event.clone());
    }
    let templates = templates();
    let facts = vec![
        templates.env(time_of(events, seq), &environment()),
        templates.permission(&permission_at(events, seq)),
    ];
    changed(&history, &By::Kernel, facts)
}

/// 样本里第 `seq` 条注入的那一块。
fn sample_fact(events: &[Event], seq: u64) -> ContextInjected {
    match events.iter().find(|event| event.seq.get() == seq) {
        Some(Event {
            body: Body::ContextInjected(fact),
            ..
        }) => fact.clone(),
        _ => panic!("样本会话里 {seq} 号不是注入的事实"),
    }
}

const READ_ONLY: &str = "<permission level=\"read_only\"/>\n";

#[test]
fn the_first_turn_injects_the_two_blocks_of_the_sample() {
    let events = events();
    assert_eq!(
        injected_after(&events, 42),
        [sample_fact(&events, 43), sample_fact(&events, 44)]
    );
}

#[test]
fn after_switching_to_read_only_only_the_permission_block_is_injected() {
    let events = events();
    let injected = injected_after(&events, 51);
    let texts: Vec<&str> = injected.iter().map(|fact| fact.text.as_str()).collect();
    assert_eq!(texts, [READ_ONLY]);
}

#[test]
fn after_undoing_that_turn_both_blocks_are_injected_again() {
    let events = events();
    let injected = injected_after(&events, 52);
    let texts: Vec<&str> = injected.iter().map(|fact| fact.text.as_str()).collect();
    assert_eq!(texts, [sample_fact(&events, 43).text.as_str(), READ_ONLY]);
}

#[test]
fn after_the_compaction_both_blocks_are_injected_again() {
    let events = events();
    let injected = injected_after(&events, 53);
    let texts: Vec<&str> = injected.iter().map(|fact| fact.text.as_str()).collect();
    assert_eq!(texts, [sample_fact(&events, 43).text.as_str(), READ_ONLY]);
}
