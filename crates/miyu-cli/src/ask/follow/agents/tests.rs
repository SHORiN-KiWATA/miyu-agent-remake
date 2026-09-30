//! 头自己数还有几个子代理没报（施工 7-9）：派出去、回报、留言，叫醒的那一轮来不来。

use serde_json::{Value, json};

use super::*;

/// 调用 `call` 的结果，带这些效果。
fn result(call: &str, effects: Value) -> Value {
    json!({"call_id": call, "status": "ok", "blocks": [], "effects": effects})
}

/// 派出去一个子代理 `job`。
fn started(job: &str, title: &str) -> Value {
    json!({"kind": "job.started", "job": job, "what": "agent", "title": title, "session": "01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91"})
}

/// 第 `seq` 条：`job` 报回来了，原因 `reason`。
fn report(seq: u64, job: &str, reason: &str) -> Value {
    json!({"seq": seq, "kind": "child.reported", "body": {"job": job, "reason": reason, "text": ""}})
}

/// 派了 j1、j2 两个子代理，还有一条后台命令 j3。
fn two() -> Agents {
    let mut agents = Agents::default();
    agents.result(&result(
        "call_10_1",
        json!([started("j1", "查 A"), started("j2", "查 B"),
            {"kind": "job.started", "job": "j3", "what": "command", "title": "跑测试"}]),
    ));
    agents
}

#[test]
fn subagents_are_owed_until_they_report_and_commands_are_not_counted() {
    let mut agents = two();
    assert_eq!(agents.owed(), 2, "后台命令不等");
    assert!(!agents.settled());
    assert_eq!(
        agents.reported(&report(20, "j1", "done"), true),
        Some(("j1".to_string(), "查 A".to_string()))
    );
    assert_eq!(agents.owed(), 1);
    // 停掉的、崩了补报的也算报过。
    agents.reported(&report(21, "j2", "aborted"), true);
    assert_eq!(agents.owed(), 0);
    assert_eq!(
        agents.reported(&report(22, "j9", "done"), false),
        None,
        "不是这次派出去的：不认，也不等它叫醒的那一轮"
    );
    assert!(agents.settled());
}

#[test]
fn a_message_after_its_report_makes_it_owe_again() {
    let mut agents = two();
    agents.reported(&report(20, "j1", "done"), false);
    agents.turn_started();
    // 第 30 条回复里的调用留了言：回报在它前面，又欠一份。
    agents.result(&result(
        "call_30_1",
        json!([{"kind": "job.messaged", "job": "j1"}]),
    ));
    assert_eq!(agents.owed(), 2);
    // 留言发出去以后、结果记下以前它就报了：算回了这句留言（内核的账本一样算）。
    agents.reported(&report(35, "j1", "done"), true);
    agents.result(&result(
        "call_30_2",
        json!([{"kind": "job.messaged", "job": "j1"}]),
    ));
    assert_eq!(agents.owed(), 1, "只剩 j2");
    // 给不是这次派的留言：不认。
    agents.result(&result(
        "call_40_1",
        json!([{"kind": "job.messaged", "job": "j7"}]),
    ));
    assert_eq!(agents.owed(), 1);
    // 调用编号读不出来的：当刚发，欠着。
    agents.result(&result(
        "bad",
        json!([{"kind": "job.messaged", "job": "j1"}]),
    ));
    assert_eq!(agents.owed(), 2);
}

#[test]
fn a_report_while_she_is_idle_brings_a_turn() {
    for (reason, by_model, wakes) in [
        ("done", false, true),
        ("stopped", false, true),
        ("stopped", true, false),
        ("undone", false, false),
        ("aborted", false, false),
        ("someday", false, true),
    ] {
        let mut agents = Agents::default();
        agents.result(&result("call_10_1", json!([started("j1", "查 A")])));
        let mut event = report(20, "j1", reason);
        event["body"]["by_model"] = json!(by_model);
        agents.reported(&event, false);
        assert_eq!(agents.settled(), !wakes, "{reason} {by_model}");
        agents.turn_started();
        assert!(agents.settled(), "等的那一轮开了头");
    }
}

#[test]
fn a_report_the_last_request_missed_brings_the_next_turn() {
    let mut agents = two();
    agents.reported(&report(20, "j1", "done"), true);
    agents.reported(&report(21, "j2", "done"), true);
    // 最后一次请求看到了第 20 条：j2 那一条没听到，内核接着开一轮。
    agents.called(&json!({"seen": 20}));
    agents.turn_ended("completed");
    assert!(!agents.settled());
    // 看的是最大的那个：压缩的摘要请求看到的在前面，不算没听到。
    let mut heard = two();
    heard.reported(&report(20, "j1", "done"), true);
    heard.reported(&report(21, "j2", "done"), true);
    heard.called(&json!({"seen": 21}));
    heard.called(&json!({"seen": 12}));
    heard.turn_ended("completed");
    assert!(heard.settled());
    agents.turn_started();
    agents.called(&json!({"seen": 25}));
    agents.turn_ended("error");
    assert!(agents.settled(), "下一轮里没有新到的：了结了");
    // 打断结束的不接着开。
    let mut agents = two();
    agents.reported(&report(20, "j1", "done"), true);
    agents.reported(&report(21, "j2", "done"), true);
    agents.turn_ended("interrupted");
    assert!(agents.settled());
    // 她自己停的：不排进这一轮的回报队。
    let mut agents = two();
    agents.reported(&report(20, "j1", "done"), true);
    agents.called(&json!({"seen": 20}));
    let mut stopped = report(21, "j2", "stopped");
    stopped["body"]["by_model"] = json!(true);
    agents.reported(&stopped, true);
    agents.turn_ended("completed");
    assert!(agents.settled());
}
