//! 有几步因为要确认、这里没人能确认被拒的（`22-命令行.md` O3，施工 4-9）：照常结束的退出码 4，用量后面再印一行；
//! 别的拒绝不算；被打断、出错的退出码照旧，那一行照样印。

use serde_json::{Value, json};

use super::*;

/// 调用 `call` 被拒了，说法是 `key`。
fn denied(call: &str, key: &str) -> Value {
    event(
        "tool.result",
        3,
        "ask-4",
        json!({"call_id": call, "status": "denied", "blocks": [], "human": {"key": key}}),
    )
}

/// 因为要确认、这里没人能确认被拒的：内核记的那一句。
fn unattended(call: &str) -> Value {
    denied(call, "core/tool-results/unattended")
}

/// 一轮照 `ended` 结束，结束之前来了这几次结果。
fn with_results(ended: &str, results: Vec<Value>) -> Vec<Value> {
    let mut turn = a_turn(ended);
    let end = turn.pop().expect("最后一条是 turn.ended");
    turn.extend(results);
    turn.push(end);
    turn
}

#[test]
fn steps_that_needed_approval_are_counted_and_the_exit_code_is_4() {
    let plan = plan(Format::Text, Language::Chinese);
    let turn = with_results("completed", vec![unattended("c1"), unattended("c2")]);
    let Fed { step, screen, .. } = feed(&plan, false, &turn);
    assert_eq!(step, Step::Done(exit::UNATTENDED));
    assert_eq!(
        screen,
        "想一想\n\n你好。\n· 输入 100 · 命中缓存 40（40%）· 输出 10\n· 2 步没做：要你确认，miyu ask 里确认不了\n",
        "用量后面再印一行"
    );
}

#[test]
fn other_denials_are_not_about_approval() {
    let plan = plan(Format::Text, Language::Chinese);
    let turn = with_results(
        "completed",
        vec![
            denied("c1", "core/tool-results/read-only"),
            denied("c2", "core/permissions/forbidden"),
        ],
    );
    let Fed { step, err, .. } = feed(&plan, false, &turn);
    assert_eq!(step, Step::Done(exit::OK));
    assert!(!err.contains("miyu ask 里确认不了"), "{err}");
}

/// 认的是被拒的那一句：状态不是 `denied` 的，说法是那一句也不算。
#[test]
fn only_a_denial_counts() {
    let plan = plan(Format::Text, Language::Chinese);
    let done = event(
        "tool.result",
        3,
        "ask-4",
        json!({"call_id": "c1", "status": "ok", "blocks": [], "human": {"key": "core/tool-results/unattended"}}),
    );
    let Fed { step, .. } = feed(&plan, false, &with_results("completed", vec![done]));
    assert_eq!(step, Step::Done(exit::OK));
}

#[test]
fn an_interrupted_turn_keeps_its_code_and_still_says_it_first() {
    let plan = plan(Format::Text, Language::Chinese);
    let turn = with_results("interrupted", vec![unattended("c1")]);
    let Fed { step, err, .. } = feed(&plan, false, &turn);
    assert_eq!(step, Step::Done(exit::INTERRUPTED));
    assert!(
        err.ends_with("· 1 步没做：要你确认，miyu ask 里确认不了\n打断了\n"),
        "{err}"
    );
}

#[test]
fn in_a_terminal_not_done_is_red() {
    let plan = plan(Format::Text, Language::Chinese);
    let turn = with_results("completed", vec![unattended("c1")]);
    let Fed { err, .. } = feed(&plan, true, &turn);
    assert!(
        err.ends_with("\x1b[90m· 1 步\x1b[31m没做\x1b[90m：要你确认，miyu ask 里确认不了\x1b[0m\n"),
        "{err:?}"
    );
}

#[test]
fn in_english_one_step_and_several() {
    let plan = plan(Format::Text, Language::English);
    let Fed { err, .. } = feed(
        &plan,
        false,
        &with_results("completed", vec![unattended("c1")]),
    );
    assert!(
        err.ends_with(
            "· 1 step not done: it needs your approval, which cannot be given in miyu ask\n"
        ),
        "{err}"
    );
    let Fed { err, .. } = feed(
        &plan,
        false,
        &with_results("completed", vec![unattended("c1"), unattended("c2")]),
    );
    assert!(
        err.ends_with(
            "· 2 steps not done: they need your approval, which cannot be given in miyu ask\n"
        ),
        "{err}"
    );
}

#[test]
fn json_tells_it_only_by_the_exit_code() {
    let plan = plan(Format::Json, Language::Chinese);
    let turn = with_results("completed", vec![unattended("c1")]);
    let Fed { step, err, .. } = feed(&plan, true, &turn);
    assert_eq!(step, Step::Done(exit::UNATTENDED));
    assert_eq!(err, "", "给脚本的：标准错误上只印出错");
}
