//! 推送读成界面关心的几样（`push.rs`）。

use serde_json::json;

use super::ToolStatus;

use super::{Block, CallError, Push};

#[test]
fn a_new_title_comes_from_meta_changed() {
    // 照 docs/designs/samples/events/session.meta_changed.jsonl。
    let event = json!({"seq": 51, "kind": "session.meta_changed", "by": {"kind": "person", "account": "alice"},
        "body": {"title": "整理 src 目录"}});
    assert_eq!(read_mine(&event), vec![Push::Title("整理 src 目录".into())]);
    let pinned =
        json!({"kind": "session.meta_changed", "by": {"kind": "person"}, "body": {"pinned": true}});
    assert!(read_mine(&pinned).is_empty(), "没带标题的不算");
}

#[test]
fn delta_start_and_text_come_with_the_model() {
    let event = json!({"kind": "model.delta", "by": {"kind": "model", "endpoint": "deepseek", "model": "deepseek-flash"},
        "body": {"seen": 3, "index": 0, "start": "reasoning"}});
    assert_eq!(
        read_mine(&event),
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
        read_mine(&event),
        vec![Push::CallFailed(CallError {
            class: "auth".into(),
            message: "no key".into(),
            status: None
        })]
    );
}

#[test]
fn a_sent_call_says_whether_its_prefix_changed() {
    let changed = json!({"kind": "model.called", "by": {"kind": "kernel"},
        "body": {"seen": 13, "request": "sha256:96e5", "messages": 1,
            "first_difference": {"part": "message", "index": 0, "role": "user"}, "result": "ok"}});
    assert_eq!(
        read_mine(&changed),
        vec![
            Push::Sent {
                seen: 13,
                changed: true,
                summary: false
            },
            Push::CallOk
        ]
    );
    let grown = json!({"kind": "model.called", "by": {"kind": "kernel"},
        "body": {"seen": 5, "request": "sha256:f8b2", "messages": 1, "result": "ok"}});
    assert_eq!(
        read_mine(&grown),
        vec![
            Push::Sent {
                seen: 5,
                changed: false,
                summary: false
            },
            Push::CallOk
        ]
    );
    // 没编码就失败的，没有 `request`：不算发出去。
    let unsent = json!({"kind": "model.called", "by": {"kind": "kernel"},
        "body": {"seen": 5, "messages": 1, "result": "error", "error": {"class": "auth", "message": "no key"}}});
    assert!(
        !read_mine(&unsent)
            .iter()
            .any(|p| matches!(p, Push::Sent { .. }))
    );
    let compacted = json!({"kind": "context.compacted", "by": {"kind": "kernel"}, "body": {}});
    assert_eq!(
        read_mine(&compacted),
        vec![Push::Compacted { clear: false }]
    );
}

#[test]
fn compaction_events_are_read() {
    use super::Compaction;
    let progress = json!({"kind": "compaction.progress", "by": {"kind": "kernel"},
        "body": {"seen": 8, "written": 3120, "expected": 40000}});
    assert_eq!(
        read_mine(&progress),
        vec![Push::Compaction(Compaction::Progress {
            written: 3120,
            expected: Some(40000)
        })]
    );
    let done = json!({"kind": "compaction.done", "by": {"kind": "kernel"},
        "body": {"seen": 8, "before": 812_300, "after": 31_000}});
    assert_eq!(
        read_mine(&done),
        vec![Push::Compaction(Compaction::Done {
            before: 812_300,
            after: 31_000
        })]
    );
    let paused = json!({"kind": "context.compaction_paused", "by": {"kind": "kernel"},
        "body": {"reason": "failures", "failures": 3}});
    assert_eq!(
        read_mine(&paused),
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
    let pushes = read_mine(&failed);
    assert!(
        pushes.contains(&Push::Compaction(Compaction::Failed(CallError {
            class: "bad_summary".into(),
            message: "the summary called a tool".into(),
            status: None
        })))
    );
    assert!(!pushes.iter().any(|p| matches!(p, Push::CallFailed(_))));
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
        aux: 0,
    };
    assert_eq!(read_mine(&event), vec![Push::Usage(usage), Push::CallOk]);
    assert_eq!(usage.input(), 40);
}

#[test]
fn turns_carry_their_numbers() {
    let started = json!({"kind": "turn.started", "turn": 42, "by": {"kind": "kernel"}, "body": {"trigger": 41}});
    assert_eq!(read_mine(&started), vec![Push::TurnStarted(42, Some(41))]);
    let reverted =
        json!({"kind": "turn.reverted", "by": {"kind": "person"}, "body": {"turns": [42, 56]}});
    assert_eq!(read_mine(&reverted), vec![Push::Reverted(vec![42, 56])]);
}

#[test]
fn tool_calls_and_results_carry_their_ids() {
    let assistant = json!({"kind": "message.assistant", "by": {"kind": "model", "endpoint": "e", "model": "m"},
        "body": {"blocks": [{"type": "text", "text": "看看"}, {"type": "tool_call", "call_id": "c1", "name": "read", "args": "{}"}]}});
    assert_eq!(read_mine(&assistant)[1], Push::Calls(vec!["c1".into()]));
    let result = json!({"kind": "tool.result", "by": {"kind": "tool"},
        "body": {"call_id": "c1", "status": "ok", "blocks": [{"type": "text", "text": "a"}, {"type": "text", "text": "b"}]}});
    assert_eq!(
        read_mine(&result),
        vec![Push::ToolResult {
            call_id: "c1".into(),
            status: ToolStatus::Ok,
            text: "a\nb".into(),
            said: None,
        }]
    );
    let end =
        json!({"kind": "model.delta", "by": {"kind": "kernel"}, "body": {"index": 1, "end": true}});
    assert_eq!(read_mine(&end), vec![Push::BlockEnd(1)]);
}

#[test]
fn events_the_screen_ignores_read_as_nothing() {
    assert!(read_mine(&json!({"kind": "session.created", "by": {"kind": "person"}})).is_empty());
}

#[test]
fn a_block_start_says_how_far_the_request_saw() {
    // 排着队的话被哪一次请求带上了，照这个认（`kernel/session.md`「排队的消息」第 1 条）。
    let start = json!({"kind": "model.delta", "by": {"kind": "kernel"},
        "body": {"seen": 44, "index": 0, "start": "text"}});
    assert_eq!(
        read_mine(&start),
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
        !read_mine(&piece)
            .iter()
            .any(|p| matches!(p, Push::Heard(_))),
        "一块开头报一次就够"
    );
}

#[test]
fn a_call_error_may_carry_its_http_status_and_a_good_call_clears_it() {
    // 施工 3-5 三补：没有状态的（连不上、流里报的）不写这一格。
    let failed = json!({"kind": "model.called", "by": {"kind": "kernel"},
        "body": {"result": "error", "error": {"class": "other", "message": "HTTP 404: no such model", "status": 404}}});
    assert_eq!(
        read_mine(&failed),
        vec![Push::CallFailed(CallError {
            class: "other".into(),
            message: "HTTP 404: no such model".into(),
            status: Some(404)
        })]
    );
    let ok = json!({"kind": "model.called", "by": {"kind": "kernel"}, "body": {"result": "ok"}});
    assert_eq!(read_mine(&ok), vec![Push::CallOk]);
    let summary = json!({"kind": "model.called", "by": {"kind": "kernel"},
        "body": {"result": "ok", "compaction": {"trigger": "auto"}}});
    assert!(
        !read_mine(&summary).contains(&Push::CallOk),
        "摘要请求成了不算这一轮的"
    );
}

#[test]
fn a_clear_is_a_compaction_marked_clear() {
    // 施工 6-8 补：清空写 `context.compacted`，`trigger` 是 `clear`。
    let clear = json!({"kind": "context.compacted", "by": {"kind": "kernel"},
        "body": {"trigger": "clear", "summary": ""}});
    assert_eq!(read_mine(&clear), vec![Push::Compacted { clear: true }]);
    let auto = json!({"kind": "context.compacted", "by": {"kind": "kernel"},
        "body": {"trigger": "auto"}});
    assert_eq!(read_mine(&auto), vec![Push::Compacted { clear: false }]);
}

/// 测试里的话都当是这个界面发的。
fn read_mine(event: &serde_json::Value) -> Vec<Push> {
    super::read(event, &|_| true)
}

#[test]
fn job_effects_reports_and_foreign_messages_are_read() {
    // 施工 7-1…7-10：工具结果的 effects 里是派出去的任务；两种回报；不是这个界面发的话单独认出来。
    use super::{JobEnd, JobReason, JobStart, Sender};
    let result = json!({"seq": 9, "kind": "tool.result", "by": {"kind": "tool", "call_id": "call_1"},
        "body": {"call_id": "call_1", "status": "ok", "blocks": [{"type": "text", "text": "j2"}],
            "effects": [{"kind": "job.started", "job": "j2", "what": "agent", "title": "查文档",
                "session": "s-child"}, {"kind": "job.messaged", "job": "j1"}]}});
    let got = read_mine(&result);
    assert_eq!(
        got[0],
        Push::JobStarted(JobStart {
            call_id: "call_1".into(),
            job: "j2".into(),
            agent: true,
            title: "查文档".into(),
            session: Some("s-child".into()),
        })
    );
    assert_eq!(got[1], Push::JobMessaged("j1".into()));
    assert!(matches!(got[2], Push::ToolResult { .. }), "工具结果照旧");
    let reported = json!({"seq": 10, "kind": "job.reported", "by": {"kind": "tool"},
        "body": {"job": "j1", "reason": "exited", "exit_code": 2, "duration_ms": 81234}});
    assert_eq!(
        read_mine(&reported),
        [Push::JobEnded(JobEnd {
            job: "j1".into(),
            reason: JobReason::Finished,
            exit_code: Some(2),
            signal: None,
            duration_ms: Some(81234),
            text: String::new(),
        })]
    );
    let child = json!({"seq": 11, "kind": "child.reported", "by": {"kind": "session", "id": "s-child"},
        "body": {"job": "j2", "session": "s-child", "reason": "undone", "text": "查到一半"}});
    let Push::JobEnded(end) = &read_mine(&child)[0] else {
        panic!("子代理的回报")
    };
    assert_eq!(
        (end.reason, end.text.as_str()),
        (JobReason::Undone, "查到一半")
    );
    // message.user：这个界面发的（cause 认得）是序号，别的带来处和字。
    let said = |by: serde_json::Value, cause: &str| {
        json!({"seq": 12, "kind": "message.user", "by": by, "cause": cause,
            "body": {"blocks": [{"type": "text", "text": "看看测试"}]}})
    };
    let mine = |c: &str| c == "tui-1";
    let person = json!({"kind": "person", "account": "admin"});
    assert_eq!(
        super::read(&said(person.clone(), "tui-1"), &mine),
        [Push::UserMessage(12)]
    );
    let foreign = |by| match super::read(&said(by, "web-7"), &mine).remove(0) {
        Push::Foreign(f) => (f.seq, f.from, f.text),
        other => panic!("{other:?}"),
    };
    assert_eq!(
        foreign(person),
        (12, Sender::Person, "看看测试".into()),
        "同一个人在别处说的"
    );
    let harness = json!({"kind": "harness", "name": "claude-code"});
    assert_eq!(foreign(harness).1, Sender::Harness("claude-code".into()));
    let session = json!({"kind": "session", "id": "s-child"});
    assert_eq!(foreign(session).1, Sender::Session("s-child".into()));
}

#[test]
fn a_subagent_effect_is_read_as_an_agent_like_the_old_name() {
    // 2026-10-01 核心把派子代理的工具从 agent 改名 subagent，旧会话里冻着旧名：两个都认成子代理。
    for what in ["agent", "subagent"] {
        let result = json!({"seq": 9, "kind": "tool.result", "by": {"kind": "tool", "call_id": "c"},
            "body": {"call_id": "c", "status": "ok", "blocks": [],
                "effects": [{"kind": "job.started", "job": "j3", "what": what, "title": "t"}]}});
        let Push::JobStarted(start) = &read_mine(&result)[0] else {
            panic!("派出去的任务")
        };
        assert!(start.agent, "{what}");
    }
}

#[test]
fn a_recap_call_only_counts_as_usage_and_the_recap_is_read() {
    // 2026-10-01 回顾（session.recap）：带 purpose 的模型调用只算用量，不算上下文、缓存、速度，出错也不算这一轮的错。
    let call = json!({"seq": 30, "kind": "model.called", "by": {"kind": "model"},
        "body": {"purpose": "recap", "request": {}, "seen": 29, "result": "error",
            "usage": {"uncached": 300, "cache_read": 0, "cache_write": 0, "output": 55},
            "duration_ms": 900, "first_token_ms": 400}});
    let got = read_mine(&call);
    assert!(
        got.iter()
            .any(|p| matches!(p, Push::AuxUsage(u) if u.uncached == 300 && u.output == 55)),
        "{got:?}"
    );
    assert!(
        !got.iter().any(|p| matches!(
            p,
            Push::Usage(_)
                | Push::Sent { .. }
                | Push::Speed { .. }
                | Push::CallFailed(_)
                | Push::CallOk
        )),
        "不算上下文、缓存、速度，出错也不算这一轮的错：{got:?}"
    );
    let recapped = json!({"seq": 31, "kind": "session.recapped", "by": {"kind": "kernel"},
        "body": {"text": "  在做回顾。  ", "upto": 28}});
    assert_eq!(read_mine(&recapped), [Push::Recapped("在做回顾。".into())]);
}
