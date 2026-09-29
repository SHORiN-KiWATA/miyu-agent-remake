//! 推送读成界面关心的几样（`push.rs`）。

use serde_json::json;

use super::ToolStatus;

use super::{Block, Push, read};

#[test]
fn a_new_title_comes_from_meta_changed() {
    // 照 docs/designs/samples/events/session.meta_changed.jsonl。
    let event = json!({"seq": 51, "kind": "session.meta_changed", "by": {"kind": "person", "account": "alice"},
        "body": {"title": "整理 src 目录"}});
    assert_eq!(read(&event), vec![Push::Title("整理 src 目录".into())]);
    let pinned =
        json!({"kind": "session.meta_changed", "by": {"kind": "person"}, "body": {"pinned": true}});
    assert!(read(&pinned).is_empty(), "没带标题的不算");
}

#[test]
fn delta_start_and_text_come_with_the_model() {
    let event = json!({"kind": "model.delta", "by": {"kind": "model", "endpoint": "deepseek", "model": "deepseek-flash"},
        "body": {"seen": 3, "index": 0, "start": "reasoning"}});
    assert_eq!(
        read(&event),
        vec![
            Push::Model {
                endpoint: "deepseek".into(),
                model: "deepseek-flash".into()
            },
            Push::Heard(3),
            Push::BlockStart {
                index: 0,
                block: Block::Reasoning
            },
        ]
    );
}

#[test]
fn a_failed_call_carries_class_and_message() {
    let event = json!({"kind": "model.called", "by": {"kind": "kernel"},
        "body": {"result": "error", "error": {"class": "auth", "message": "no key"}}});
    assert_eq!(
        read(&event),
        vec![Push::CallFailed {
            class: "auth".into(),
            message: "no key".into()
        }]
    );
}

#[test]
fn a_sent_call_says_whether_its_prefix_changed() {
    let changed = json!({"kind": "model.called", "by": {"kind": "kernel"},
        "body": {"seen": 13, "request": "sha256:96e5", "messages": 1,
            "first_difference": {"part": "message", "index": 0, "role": "user"}, "result": "ok"}});
    assert_eq!(
        read(&changed),
        vec![Push::Sent {
            seen: 13,
            changed: true,
            summary: false
        }]
    );
    let grown = json!({"kind": "model.called", "by": {"kind": "kernel"},
        "body": {"seen": 5, "request": "sha256:f8b2", "messages": 1, "result": "ok"}});
    assert_eq!(
        read(&grown),
        vec![Push::Sent {
            seen: 5,
            changed: false,
            summary: false
        }]
    );
    // 没编码就失败的，没有 `request`：不算发出去。
    let unsent = json!({"kind": "model.called", "by": {"kind": "kernel"},
        "body": {"seen": 5, "messages": 1, "result": "error", "error": {"class": "auth", "message": "no key"}}});
    assert!(!read(&unsent).iter().any(|p| matches!(p, Push::Sent { .. })));
    let compacted = json!({"kind": "context.compacted", "by": {"kind": "kernel"}, "body": {}});
    assert_eq!(read(&compacted), vec![Push::Compacted]);
}

#[test]
fn compaction_events_are_read() {
    use super::Compaction;
    let progress = json!({"kind": "compaction.progress", "by": {"kind": "kernel"},
        "body": {"seen": 8, "written": 3120, "expected": 40000}});
    assert_eq!(
        read(&progress),
        vec![Push::Compaction(Compaction::Progress {
            written: 3120,
            expected: Some(40000)
        })]
    );
    let done = json!({"kind": "compaction.done", "by": {"kind": "kernel"},
        "body": {"seen": 8, "before": 812_300, "after": 31_000}});
    assert_eq!(
        read(&done),
        vec![Push::Compaction(Compaction::Done {
            before: 812_300,
            after: 31_000
        })]
    );
    let paused = json!({"kind": "context.compaction_paused", "by": {"kind": "kernel"},
        "body": {"reason": "failures", "failures": 3}});
    assert_eq!(
        read(&paused),
        vec![Push::Compaction(Compaction::Paused {
            reason: "failures".into(),
            failures: Some(3),
            entry: None
        })]
    );
    // 摘要请求出错：说压缩失败，不算这一轮的出错；它也是摘要请求，数缓存断裂时认得出。
    let failed = json!({"kind": "model.called", "by": {"kind": "kernel"},
        "body": {"seen": 8, "request": "sha256:ab", "messages": 3, "compaction": "auto",
            "result": "error", "error": {"class": "bad_summary", "message": "the summary called a tool"}}});
    let pushes = read(&failed);
    assert!(pushes.contains(&Push::Compaction(Compaction::Failed {
        class: "bad_summary".into(),
        message: "the summary called a tool".into()
    })));
    assert!(!pushes.iter().any(|p| matches!(p, Push::CallFailed { .. })));
    assert!(pushes.contains(&Push::Sent {
        seen: 8,
        changed: false,
        summary: true
    }));
}

#[test]
fn a_call_reports_its_usage() {
    let event = json!({"kind": "model.called", "by": {"kind": "kernel"},
        "body": {"result": "ok", "usage": {"uncached": 10, "cache_read": 30, "cache_write": 0, "output": 5}}});
    let usage = super::Usage {
        uncached: 10,
        cache_read: 30,
        cache_write: 0,
        output: 5,
    };
    assert_eq!(read(&event), vec![Push::Usage(usage)]);
    assert_eq!(usage.input(), 40);
}

#[test]
fn turns_carry_their_numbers() {
    let started = json!({"kind": "turn.started", "turn": 42, "by": {"kind": "kernel"}, "body": {"trigger": 41}});
    assert_eq!(read(&started), vec![Push::TurnStarted(42, Some(41))]);
    let reverted =
        json!({"kind": "turn.reverted", "by": {"kind": "person"}, "body": {"turns": [42, 56]}});
    assert_eq!(read(&reverted), vec![Push::Reverted(vec![42, 56])]);
}

#[test]
fn tool_calls_and_results_carry_their_ids() {
    let assistant = json!({"kind": "message.assistant", "by": {"kind": "model", "endpoint": "e", "model": "m"},
        "body": {"blocks": [{"type": "text", "text": "看看"}, {"type": "tool_call", "call_id": "c1", "name": "read", "args": "{}"}]}});
    assert_eq!(read(&assistant)[1], Push::Calls(vec!["c1".into()]));
    let result = json!({"kind": "tool.result", "by": {"kind": "tool"},
        "body": {"call_id": "c1", "status": "ok", "blocks": [{"type": "text", "text": "a"}, {"type": "text", "text": "b"}]}});
    assert_eq!(
        read(&result),
        vec![Push::ToolResult {
            call_id: "c1".into(),
            status: ToolStatus::Ok,
            text: "a\nb".into(),
            said: None,
        }]
    );
    let end =
        json!({"kind": "model.delta", "by": {"kind": "kernel"}, "body": {"index": 1, "end": true}});
    assert_eq!(read(&end), vec![Push::BlockEnd(1)]);
}

#[test]
fn events_the_screen_ignores_read_as_nothing() {
    assert!(read(&json!({"kind": "session.created", "by": {"kind": "person"}})).is_empty());
}

#[test]
fn a_block_start_says_how_far_the_request_saw() {
    // 排着队的话被哪一次请求带上了，照这个认（`kernel/session.md`「排队的消息」第 1 条）。
    let start = json!({"kind": "model.delta", "by": {"kind": "kernel"},
        "body": {"seen": 44, "index": 0, "start": "text"}});
    assert_eq!(
        read(&start),
        vec![
            Push::Heard(44),
            Push::BlockStart {
                index: 0,
                block: crate::core::Block::Text
            }
        ]
    );
    let piece = json!({"kind": "model.delta", "by": {"kind": "kernel"},
        "body": {"seen": 44, "index": 0, "text": "我先"}});
    assert!(
        !read(&piece).iter().any(|p| matches!(p, Push::Heard(_))),
        "一块开头报一次就够"
    );
}
