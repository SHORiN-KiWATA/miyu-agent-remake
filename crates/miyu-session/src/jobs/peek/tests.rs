//! 子代理在做什么（施工 7-4）：最近的回答、这一轮完没完、这一步在跑哪几件工具。

use super::*;

/// 一条事件：序号 `seq`，种类 `kind`，内容 `body`。
fn event(seq: u64, kind: &str, body: &str) -> Event {
    let line = format!(
        r#"{{"seq":{seq},"at":"2026-09-25T07:00:00.000Z","kind":"{kind}","by":{{"kind":"kernel"}},"body":{body}}}"#
    );
    Event::from_line(&line).unwrap_or_else(|error| panic!("{line}: {error}"))
}

fn reply(seq: u64, text: &str, calls: &[(&str, &str)]) -> Event {
    let mut blocks = Vec::new();
    if !text.is_empty() {
        blocks.push(serde_json::json!({"type": "text", "text": text}));
    }
    for (call, name) in calls {
        blocks.push(
            serde_json::json!({"type": "tool_call", "call_id": call, "name": name, "args": "{}"}),
        );
    }
    let body = serde_json::json!({"blocks": blocks, "seen": seq - 1});
    event(seq, "message.assistant", &body.to_string())
}

fn result(seq: u64, call: &str) -> Event {
    let body = serde_json::json!({"call_id": call, "status": "ok", "blocks": []});
    event(seq, "tool.result", &body.to_string())
}

#[test]
fn a_fresh_session_has_said_nothing() {
    assert_eq!(peek(&[]), Peek::default());
}

#[test]
fn the_running_tools_are_the_calls_without_results() {
    let events = [
        event(2, "turn.started", "{}"),
        reply(
            3,
            "Reading the tests first.",
            &[("call_3_1", "read"), ("call_3_2", "grep")],
        ),
        result(4, "call_3_1"),
    ];
    assert_eq!(
        peek(&events),
        Peek {
            reply: Some("Reading the tests first.".to_string()),
            this_turn: true,
            working: true,
            doing: vec!["grep".to_string()],
        }
    );
}

#[test]
fn an_ended_turn_is_not_working_and_a_new_turn_has_no_reply_yet() {
    let mut events = vec![
        event(2, "turn.started", "{}"),
        reply(3, "Done: 3 files.", &[]),
        event(4, "turn.ended", r#"{"reason":"completed"}"#),
    ];
    let done = peek(&events);
    assert_eq!((done.working, done.this_turn), (false, true));
    assert!(done.doing.is_empty());
    events.push(event(5, "turn.started", "{}"));
    events.push(reply(6, "", &[("call_6_1", "shell")]));
    let again = peek(&events);
    assert_eq!(
        again.reply.as_deref(),
        Some("Done: 3 files."),
        "最近的回答照旧是上一轮的"
    );
    assert!(!again.this_turn, "这一轮还没说过话");
    assert_eq!(again.doing, ["shell"]);
}
