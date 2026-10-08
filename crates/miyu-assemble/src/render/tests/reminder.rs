//! 角色扮演提示（施工 P-1 补，`08-上下文投影.md` C2 唯一的例外）：内核在回合开始时注入的这一块排在触发的那句后面，开始时
//! 注入的别的事实照旧在触发前面；以后的请求里位置不变；触发不在这一段里的照先后；模块注入的同类块不挪。

use super::*;

const MEMORY: &str = r#"{"kind":"module","id":"memory"}"#;

/// 开一轮：环境、提示（内核），召回（模块，挂接点跑完才来）。
fn opening(log: &mut Log, trigger: u64) {
    log.start(trigger);
    log.fact("<env/>");
    log.fact_by(KERNEL, "reminder", "<r/>");
    log.fact_by(MEMORY, "recall", "<m/>");
}

#[test]
fn the_reminder_follows_the_trigger_and_the_other_facts_stay_before_it() {
    let mut log = Log::new();
    let hi = log.say("hi");
    opening(&mut log, hi);
    assert_eq!(rendered(&log), ["user: <env/> | <m/> | hi | <r/>"]);
}

#[test]
fn it_keeps_its_place_in_the_requests_after() {
    let mut log = Log::new();
    let hi = log.say("look at src");
    opening(&mut log, hi);
    let call = log.reply_calling("looking");
    log.result(&call, "ok", "lib.rs");
    assert_eq!(
        rendered(&log),
        [
            "user: <env/> | <m/> | look at src | <r/>".to_string(),
            "assistant: looking | [call read]".to_string(),
            format!("tool {call} ok: lib.rs"),
        ]
    );
}

#[test]
fn a_message_before_or_after_the_turn_started_keeps_its_order() {
    let mut log = Log::new();
    log.say("one");
    let two = log.say("two");
    opening(&mut log, two);
    log.say("three");
    assert_eq!(
        rendered(&log),
        ["user: one | <env/> | <m/> | two | <r/> | three"]
    );
}

#[test]
fn without_its_trigger_in_the_segment_it_keeps_its_place() {
    let mut log = Log::new();
    log.say("hi");
    let fired = log.push(
        r#"{"kind":"module","id":"timer"}"#,
        "ext.timer.fired",
        r#"{"name":"standup"}"#,
    );
    opening(&mut log, fired);
    assert_eq!(rendered(&log), ["user: hi | <env/> | <r/> | <m/>"]);
}

#[test]
fn a_module_block_of_the_same_kind_is_not_moved() {
    let mut log = Log::new();
    let hi = log.say("hi");
    log.start(hi);
    log.fact_by(MEMORY, "reminder", "<x/>");
    assert_eq!(rendered(&log), ["user: <x/> | hi"]);
}
