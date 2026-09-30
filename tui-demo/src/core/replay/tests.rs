//! 补发来的事件读成看着流出来时一样的推送（蓝图「会话列表 `/sessions`」第 6 条）。

use jiff::Timestamp;
use serde_json::{Value, json};

use super::Replay;
use crate::core::{Block, Push};

fn t(s: &str) -> Option<Timestamp> {
    Some(s.parse().unwrap())
}

fn said() -> Value {
    json!({"seq":45,"at":"2026-09-25T07:04:07.890Z","kind":"message.assistant","turn":42,
        "by":{"kind":"model","endpoint":"deepseek","model":"deepseek-v4"},
        "body":{"blocks":[{"type":"reasoning","text":"先看目录"},{"type":"text","text":"我看一下。"},
            {"type":"tool_call","call_id":"call_1","name":"read","args":"{\"path\":\"src\"}"}],"seen":44}})
}

fn called() -> Value {
    json!({"seq":46,"at":"2026-09-25T07:04:08.000Z","kind":"model.called","turn":42,"by":{"kind":"kernel"},
        "body":{"seen":44,"usage":{"uncached":10,"cache_read":0,"cache_write":0,"output":5},
            "first_token_ms":500,"duration_ms":4000,
            "blocks":[{"start_ms":500,"end_ms":2500},{"start_ms":2600,"end_ms":3000},{"start_ms":3100,"end_ms":3900}],"result":"ok"}})
}

/// 只留开块、写字、收块和钟，看顺序。
fn shape(pushes: &[Push]) -> Vec<String> {
    pushes
        .iter()
        .filter_map(|p| match p {
            Push::Clock(Some(t)) => Some(format!("clock {}", t.strftime("%S%.3f"))),
            Push::Clock(None) => Some("clock now".into()),
            Push::BlockStart { index, block } => Some(format!("start {index} {block:?}")),
            Push::Delta { index, text } => Some(format!("delta {index} {text}")),
            Push::BlockEnd(index) => Some(format!("end {index}")),
            Push::Calls(ids) => Some(format!("calls {ids:?}")),
            _ => None,
        })
        .collect()
}

#[test]
fn what_a_person_said_is_drawn_as_yours_with_its_time() {
    let mut replay = Replay::default();
    let user = json!({"seq":41,"at":"2026-09-25T07:04:05.123Z","kind":"message.user","by":{"kind":"person","account":"alice"},
        "cause":"cmd-7f3a","body":{"blocks":[{"type":"text","text":"看看 src 目录"}]}});
    assert_eq!(
        replay.read(&user),
        vec![
            Push::Clock(t("2026-09-25T07:04:05.123Z")),
            Push::Said {
                seq: 41,
                text: "看看 src 目录".into()
            }
        ]
    );
}

#[test]
fn an_answer_waits_for_its_call_and_each_block_takes_the_time_it_took() {
    let mut replay = Replay::default();
    assert!(replay.read(&said()).is_empty(), "等它的 model.called");
    let out = replay.read(&called());
    // 请求 07:04:04.000 发出去（08.000 减 4000 毫秒），第一块 04.500 到 06.500。
    assert_eq!(
        shape(&out),
        [
            "clock 04.500",
            "start 0 Reasoning",
            "delta 0 先看目录",
            "clock 06.500",
            "end 0",
            "clock 06.600",
            "start 1 Text",
            "delta 1 我看一下。",
            "clock 07.000",
            "end 1",
            "clock 07.100",
            "start 2 ToolCall(\"read\")",
            "delta 2 {\"path\":\"src\"}",
            "clock 07.900",
            "end 2",
            "clock 07.890",
            "calls [\"call_1\"]",
            "clock 08.000",
        ]
    );
    assert!(out.iter().any(|p| matches!(p, Push::Usage(_))), "用量照记");
    assert!(matches!(out[0], Push::Clock(_)));
    let _ = Block::Text;
}

#[test]
fn an_answer_without_its_call_uses_its_own_time() {
    let mut replay = Replay::default();
    replay.read(&said());
    let result = json!({"seq":47,"at":"2026-09-25T07:04:09.000Z","kind":"tool.result","turn":42,"by":{"kind":"kernel"},
        "body":{"call_id":"call_1","status":"ok","blocks":[{"type":"text","text":"lib.rs"}]}});
    let out = replay.read(&result);
    let shape = shape(&out);
    assert_eq!(shape[0], "clock 07.890", "没有起止时刻：照落盘的时刻");
    assert!(shape.contains(&"calls [\"call_1\"]".to_string()));
    assert!(
        out.iter().any(|p| matches!(p, Push::ToolResult { .. })),
        "后面那一条照读"
    );
}

#[test]
fn finishing_reads_what_is_left_and_goes_back_to_now() {
    let mut replay = Replay::default();
    replay.read(&said());
    let shape = shape(&replay.finish());
    assert_eq!(shape.last().map(String::as_str), Some("clock now"));
    assert!(shape.iter().any(|s| s == "start 1 Text"));
}
