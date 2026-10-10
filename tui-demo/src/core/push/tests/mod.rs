//! 推送读成界面关心的几样（`push.rs`）。

use serde_json::json;

use super::ToolStatus;

use super::{Block, CallError, Push};

mod jobs;

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
fn a_new_workspace_comes_from_workspace_changed() {
    // 照 docs/designs/samples/events/session.workspace_changed.jsonl（核心 9-7 上）。
    let event = json!({"seq": 12, "kind": "session.workspace_changed", "by": {"kind": "person", "account": "alice"},
        "body": {"cwd": "/home/alice/proj", "dirs": ["/home/alice/docs"]}});
    assert_eq!(
        read_mine(&event),
        vec![Push::Workspace("/home/alice/proj".into())]
    );
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
            expected: Some(40000),
            manual: None,
        })]
    );
    let manual = json!({"kind": "compaction.progress", "by": {"kind": "kernel"},
        "body": {"seen": 8, "written": 0, "expected": 40000, "trigger": "manual"}});
    assert!(matches!(
        read_mine(&manual)[..],
        [Push::Compaction(Compaction::Progress {
            manual: Some(true),
            ..
        })]
    ));
    let done = json!({"kind": "compaction.done", "by": {"kind": "kernel"},
        "body": {"seen": 8, "before": 812_300, "after": 31_000}});
    assert_eq!(
        read_mine(&done),
        vec![Push::Compaction(Compaction::Done {
            before: 812_300,
            after: 31_000,
            prepared: false,
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
    // 没有 `cost` 的：算一次没有价格（核心 8-15）。
    assert_eq!(
        read_mine(&event),
        vec![Push::Usage(usage), Push::Billed(None), Push::CallOk]
    );
    assert_eq!(usage.input(), 40);
    let priced = json!({"kind": "model.called", "by": {"kind": "kernel"},
        "body": {"result": "ok", "usage": {"uncached": 1, "cache_read": 0, "cache_write": 0, "output": 1},
                 "cost": {"amount": 0.0002, "currency": "USD", "multiplier": 1, "source": "local"}}});
    let cost = crate::core::Cost {
        amount: 0.0002,
        currency: "USD".into(),
    };
    assert!(read_mine(&priced).contains(&Push::Billed(Some(cost))));
    let interrupted = json!({"kind": "model.called", "by": {"kind": "kernel"}, "body": {"result": "interrupted"}});
    assert!(
        !read_mine(&interrupted)
            .iter()
            .any(|p| matches!(p, Push::Billed(_))),
        "没报用量的不算一次请求的钱"
    );
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
fn a_tool_result_with_images_also_hands_over_their_blobs() {
    // 读了图片文件的 `read`：结果里的图片块（核心 2026-10-09 告知形状）另交一条，时间线照它把图接在这一步下面。
    let result = json!({"kind": "tool.result", "by": {"kind": "tool"},
        "body": {"call_id": "c1", "status": "ok", "blocks": [
            {"type": "text", "text": "cat.png"},
            {"type": "image", "blob": "sha256:aa", "media_type": "image/png"},
            {"type": "image", "blob": "sha256:bb", "media_type": "image/jpeg"}]}});
    let got = read_mine(&result);
    assert!(matches!(&got[0], Push::ToolResult { text, .. } if text == "cat.png"));
    assert_eq!(
        got[1],
        Push::ToolImages {
            call_id: "c1".into(),
            blobs: vec!["sha256:aa".into(), "sha256:bb".into()],
        }
    );
    let plain = json!({"kind": "tool.result", "by": {"kind": "tool"},
        "body": {"call_id": "c2", "status": "ok", "blocks": [{"type": "text", "text": "a"}]}});
    assert_eq!(read_mine(&plain).len(), 1, "没有图的不另交");
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
pub(super) fn read_mine(event: &serde_json::Value) -> Vec<Push> {
    super::read(event, &|_| true)
}

#[test]
fn a_summary_prepared_in_the_background_only_counts_as_usage() {
    // 核心 6-11 上：后台提前压的摘要请求 `purpose` 是 `compaction`，不带 `compaction` 那一格；出错了不算压缩失败，也不算
    // 这一轮的错，只算用量。
    let call = json!({"seq": 40, "kind": "model.called", "by": {"kind": "model"},
        "body": {"purpose": "compaction", "request": {}, "seen": 120, "result": "error",
            "usage": {"uncached": 900, "cache_read": 0, "cache_write": 0, "output": 80},
            "error": {"class": "server", "message": "boom"}}});
    let got = read_mine(&call);
    assert!(
        got.iter()
            .any(|p| matches!(p, Push::AuxUsage(u) if u.uncached == 900)),
        "{got:?}"
    );
    assert!(
        !got.iter().any(|p| matches!(
            p,
            Push::Usage(_) | Push::Sent { .. } | Push::CallFailed(_) | Push::Compaction(_)
        )),
        "{got:?}"
    );
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

#[test]
fn a_failed_call_says_which_endpoint_it_tried() {
    // 换端点那一行的「原来的」照它写（核心 8-9）。
    let called = json!({"seq": 9, "kind": "model.called", "by": {"kind": "kernel"},
        "body": {"seen": 3, "endpoint": "bad", "model": "m", "result": "error",
            "error": {"class": "retryable", "message": "connection refused"}}});
    let got = read_mine(&called);
    let tried = got.iter().position(
        |p| matches!(p, Push::Tried { endpoint, model } if endpoint == "bad" && model == "m"),
    );
    let failed = got.iter().position(|p| matches!(p, Push::CallFailed(_)));
    assert!(
        tried.is_some() && tried < failed,
        "先说试的哪个，再说出错：{got:?}"
    );
}

#[test]
fn a_failover_model_change_is_read_with_its_limits() {
    // 核心 8-9：瞬时的 `model.changed`，`why` 是 `failover`；没值的格不写。
    let changed = json!({"at":"2026-09-25T08:20:44.900Z","kind":"model.changed","turn":131,"by":{"kind":"kernel"},
        "body":{"ref":"@duo","endpoint":"bigmodel","model":"glm-5.3-flash",
            "limits":{"window":200000,"compaction_line":167000},"why":"failover",
            "effort":{"level":"high","from":"personal"}}});
    assert_eq!(
        read_mine(&changed),
        vec![Push::ModelChanged {
            endpoint: Some("bigmodel".into()),
            model: Some("glm-5.3-flash".into()),
            limits: Some(crate::core::Limits {
                window: Some(200_000),
                compaction_line: Some(167_000)
            }),
            failover: true,
            reference: Some("@duo".into()),
            effort: Some("high".into()),
        }]
    );
}

#[test]
fn a_policy_change_with_only_the_policy_draws_nothing() {
    // 核心 P-1 再补：改了人格的文件，下一个回合开头内核推一条只带 `policy` 的，`permission`、`model` 都没有。
    let event = json!({"kind": "session.policy_changed", "turn": 7, "by": {"kind": "kernel"},
        "body": {"policy": "sha256:0123"}});
    assert!(read_mine(&event).is_empty(), "不改权限、不换模型");
}

#[test]
fn venue_events_are_skipped_and_a_venue_on_a_message_changes_nothing() {
    // 核心 O-13 上：场所会话多两种事件、`message.user` 多一格 `venue`（本机的头本来看不到这些会话）；O-14 下多
    // `turn.joined`（桥把群里的几条并进这一轮）。认不得的跳过，不画。
    let mine = |c: &str| c == "tui-1";
    for kind in ["venue.recalled", "venue.delivered", "turn.joined"] {
        let event =
            json!({"seq": 3, "kind": kind, "by": {"kind": "kernel"}, "body": {"message": "m-1"}});
        assert!(super::read(&event, &mine).is_empty(), "{kind}");
    }
    let said = json!({"seq": 4, "kind": "message.user", "by": {"kind": "person", "account": "admin"},
        "cause": "tui-1", "body": {"blocks": [{"type": "text", "text": "在吗"}], "venue": {"id": "qq-group-1"}}});
    assert_eq!(super::read(&said, &mine), [Push::UserMessage(4)]);
    // 核心 O-2 中：扩展登记、关掉工具以后，下一个回合开头多一条只带 `policy` 的 `session.policy_changed`，不画。
    let tools = json!({"seq": 5, "kind": "session.policy_changed", "by": {"kind": "kernel"},
        "body": {"policy": {"tools": {"off": ["web_fetch"]}}}});
    assert!(super::read(&tools, &mine).is_empty());
}
