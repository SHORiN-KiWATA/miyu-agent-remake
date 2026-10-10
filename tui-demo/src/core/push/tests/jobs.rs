//! 后台任务、子代理、别处来的话（施工 7-1…7-10、核心 T-1 下）：工具结果的 `effects`、两种回报、空下来的会话。

use serde_json::json;

use super::read_mine;
use crate::core::push::Push;

#[test]
fn job_effects_reports_and_foreign_messages_are_read() {
    // 施工 7-1…7-10：工具结果的 effects 里是派出去的任务；两种回报；不是这个界面发的话单独认出来。
    use crate::core::push::{JobEnd, JobReason, JobStart, Sender};
    let result = json!({"seq": 9, "kind": "tool.result", "by": {"kind": "tool", "call_id": "call_1"},
        "body": {"call_id": "call_1", "status": "ok", "blocks": [{"type": "text", "text": "j2"}],
            "effects": [{"kind": "job.started", "job": "j2", "what": "subagent", "title": "查文档",
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
            foreground: false,
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
        crate::core::push::read(&said(person.clone(), "tui-1"), &mine),
        [Push::UserMessage(12)]
    );
    let foreign = |by| match crate::core::push::read(&said(by, "web-7"), &mine).remove(0) {
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
fn only_a_subagent_effect_is_read_as_an_agent() {
    // 2026-10-01 核心把派子代理的工具从 agent 改名 subagent，同一天项目主人定不认旧名、不留兼容。
    for (what, agent) in [("subagent", true), ("agent", false)] {
        let result = json!({"seq": 9, "kind": "tool.result", "by": {"kind": "tool", "call_id": "c"},
            "body": {"call_id": "c", "status": "ok", "blocks": [],
                "effects": [{"kind": "job.started", "job": "j3", "what": what, "title": "t"}]}});
        let Push::JobStarted(start) = &read_mine(&result)[0] else {
            panic!("派出去的任务")
        };
        assert_eq!(start.agent, agent, "{what}");
    }
}

#[test]
fn a_peer_going_idle_is_read_with_its_reason_and_last_line() {
    // 核心 C-6「空了告诉我」：`peer.idle`，`status` 可以没有。
    let idle = json!({"seq": 30, "kind": "peer.idle", "by": {"kind": "session", "id": "w"},
        "cause": "w/idle/m/1", "body": {"session": "0192f3a0-1111-7abc-8def-001122334455",
        "reason": "idle", "status": "算完了，结果是 55"}});
    assert_eq!(
        read_mine(&idle),
        vec![Push::PeerIdle {
            session: "0192f3a0-1111-7abc-8def-001122334455".into(),
            reason: "idle".into(),
            status: Some("算完了，结果是 55".into()),
        }]
    );
    let gone = json!({"seq": 31, "kind": "peer.idle", "by": {"kind": "kernel"},
        "body": {"session": "s", "reason": "gone"}});
    assert_eq!(
        read_mine(&gone),
        vec![Push::PeerIdle {
            session: "s".into(),
            reason: "gone".into(),
            status: None
        }]
    );
}

#[test]
fn a_foreground_subagent_is_marked() {
    // 核心 T-1 下：预设关了「后台运行」时子代理在前台跑，`job.started` 带 `foreground: true`，不当后台任务画。
    let result = json!({"seq": 9, "kind": "tool.result", "by": {"kind": "tool", "call_id": "call_1"},
        "body": {"call_id": "call_1", "status": "ok", "blocks": [{"type": "text", "text": "好了"}],
            "effects": [{"kind": "job.started", "job": "j3", "what": "subagent", "title": "查文档",
                "session": "s-child", "foreground": true}]}});
    let Push::JobStarted(start) = &read_mine(&result)[0] else {
        panic!("派了一个任务");
    };
    assert!(start.foreground);
    let listed = crate::core::push::JobStart::listed(
        &json!({"job": "j3", "what": "subagent", "title": "x"}),
    );
    assert!(!listed.foreground, "不写的是后台的");
}
