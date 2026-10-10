//! 这一轮不说话了（施工 O-26，`onebot.md` 第一条「提供者和不说话」第 3 条）：推来的事件照原样的 JSON 交进来。

use serde_json::{Value, json};

use super::Quiet;

/// 她在回合 `turn` 的一条回复，块是 `blocks`。
fn reply(turn: u64, blocks: Value) -> Value {
    json!({"seq": 10, "kind": "message.assistant", "turn": turn, "body": {"blocks": blocks}})
}

/// 一块字。
fn text(text: &str) -> Value {
    json!({"type": "text", "text": text})
}

/// 一块调用：调 `name`。
fn call(name: &str) -> Value {
    json!({"type": "tool_call", "call_id": "c1", "name": name, "args": "{\"reason\":\"x\"}"})
}

/// 回合 `turn` 结束了。
fn ended(turn: u64) -> Value {
    json!({"seq": 20, "kind": "turn.ended", "turn": turn, "body": {"reason": "done"}})
}

#[test]
fn a_reply_that_calls_skip_reply_hushes_the_rest_of_its_turn() {
    let mut quiet = Quiet::default();
    assert!(
        !quiet.heard("s1", &reply(5, json!([text("先说一句")]))),
        "调用以前的照发"
    );
    assert!(
        quiet.heard("s1", &reply(5, json!([text("不该发"), call("skip_reply")]))),
        "同一条里的字不发"
    );
    assert!(
        quiet.heard("s1", &reply(5, json!([text("也不发")]))),
        "这一轮以后的不发"
    );
    assert!(
        !quiet.heard("s1", &reply(9, json!([text("别的回合")]))),
        "别的回合照发"
    );
    assert!(
        !quiet.heard("s2", &reply(5, json!([text("别的会话")]))),
        "别的会话照发：回合编号各数各的"
    );
}

#[test]
fn other_calls_and_plain_words_do_not() {
    let mut quiet = Quiet::default();
    assert!(!quiet.heard("s1", &reply(5, json!([text("x"), call("no_such_tool")]))));
    assert!(!quiet.heard("s1", &reply(5, json!([text("skip_reply")]))));
    assert!(!quiet.heard(
        "s1",
        &reply(5, json!([{"type": "reasoning", "text": "skip_reply"}]))
    ));
    assert!(!quiet.heard("s1", &reply(5, json!([text("y")]))));
}

#[test]
fn the_end_of_the_turn_clears_it() {
    let mut quiet = Quiet::default();
    assert!(quiet.heard("s1", &reply(5, json!([call("skip_reply")]))));
    assert!(quiet.heard("s1", &reply(7, json!([call("skip_reply")]))));
    assert!(!quiet.heard("s1", &ended(5)), "不是她的回复");
    assert!(
        !quiet.heard("s1", &reply(5, json!([text("x")]))),
        "那一轮清掉了"
    );
    assert!(
        quiet.heard("s1", &reply(7, json!([text("y")]))),
        "别的回合的不清"
    );
    assert!(!quiet.heard("s2", &ended(7)), "别的会话的结束");
    assert!(
        quiet.heard("s1", &reply(7, json!([text("z")]))),
        "别的会话的结束不清这一个"
    );
}

#[test]
fn events_without_a_turn_are_not_looked_at() {
    let mut quiet = Quiet::default();
    let mut lone = reply(5, json!([call("skip_reply")]));
    lone.as_object_mut().expect("是对象").remove("turn");
    assert!(!quiet.heard("s1", &lone));
    assert!(!quiet.heard("s1", &reply(5, json!([text("x")]))));
}
