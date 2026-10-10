//! 桥当提供者（施工 O-26，`onebot.md` 第一条「提供者和不说话」第 1、2 条）：进程里的桥经内存里的管道连核心（本机套接字上的
//! 核心不认提供者，`provide` 回 `not_a_provider`，桥照样收发）。桥连上核心头一个发 `provide`；测试照核心反向调用的样子推请求：
//! `tool.call` 照工具答（`skip_reply` 答出厂的那一句，不认识的回 `error: true`），别的方法回「没有这个方法」，`tool.cancel`
//! 不回。说明、答的两句是出厂数据：写坏了、要了别的字段、不在的桥起不来。

use std::time::Duration;

use serde_json::{Value, json};

use miyu_chat::Source;
use miyu_config::problem::Code;
use miyu_onebot::rules::Factory;
use miyu_session::testkit::{Play, Script};
use miyu_store::resources::ResourceRoot;

use crate::rules::{clean, copied_resources};
use crate::support::*;

/// 出厂的资源 `software/onebot/<name>` 原样。
fn shipped(name: &str) -> String {
    std::fs::read_to_string(resources().join("software/onebot").join(name)).expect("读得出")
}

/// 照核心反向调用的样子拼一次 `tool.call`：编号 `id`，工具 `tool`。
fn tool_call(id: &str, tool: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tool.call",
        "params": {
            "session": "0190f3a1-0000-7000-8000-000000000001",
            "call_id": "c1",
            "tool": tool,
            "args": {"reason": "not for me"},
            "cwd": "/",
        },
    })
}

/// 等到桥回了编号是 `id` 的回应。
async fn answer(relay: &Relay, id: &str) -> Value {
    within("桥回核心", async {
        loop {
            if let Some(found) = relay.answers().into_iter().find(|one| one["id"] == id) {
                return found;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
}

#[tokio::test]
async fn the_bridge_provides_first_and_answers_the_cores_requests() {
    let home = Home::new(&Script::new([Play::Says("在。")]));
    let (serve, relay) = serve_pushing(home.root.clone(), settings());
    let bridge = start(serve).await;
    within("桥问核心", async {
        while relay.asked().is_empty() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    assert_eq!(
        relay.asked().first().map(String::as_str),
        Some("provide"),
        "握手以后头一个登记：{:?}",
        relay.asked()
    );
    relay.request(json!({
        "jsonrpc": "2.0",
        "method": "tool.cancel",
        "params": {"session": "0190f3a1-0000-7000-8000-000000000001", "call_id": "c0"},
    }));
    relay.request(tool_call("core-1", "skip_reply"));
    relay.request(tool_call("core-2", "recall"));
    relay.request(json!({"jsonrpc": "2.0", "id": "core-3", "method": "tool.peek", "params": {}}));
    assert_eq!(
        answer(&relay, "core-1").await,
        json!({"jsonrpc": "2.0", "id": "core-1", "result": {
            "blocks": [{"type": "text", "text": shipped("tool-results/skipped.txt")}],
            "error": false,
        }})
    );
    let unknown = shipped("tool-results/unknown.txt").replace("{name}", "recall");
    assert_eq!(
        answer(&relay, "core-2").await,
        json!({"jsonrpc": "2.0", "id": "core-2", "result": {
            "blocks": [{"type": "text", "text": unknown}],
            "error": true,
        }})
    );
    let refused = answer(&relay, "core-3").await;
    assert_eq!(refused["error"]["code"], -32601, "{refused}");
    assert_eq!(
        refused["error"]["data"]["reason"], "unknown_method",
        "{refused}"
    );
    // `tool.cancel` 不回；桥照样收发。
    let mut napcat = admin_napcat(bridge.port).await;
    napcat.admin_says(1, "在吗").await;
    assert_eq!(napcat.reply().await, "在。", "provide 被拒了，话照说");
    assert_eq!(relay.answers().len(), 3, "{:?}", relay.answers());
    bridge.stop().await.expect("停得下");
}

#[test]
fn the_shipped_tools_are_read_and_broken_ones_stop_the_bridge() {
    assert!(Factory::load(&ResourceRoot::at(resources())).is_ok());
    let resources = copied_resources();
    let software = resources.join("software/onebot");
    for (file, text, wanted) in [
        (
            "tools/skip_reply.json",
            "{\"description\": \"x\"}",
            "parameters",
        ),
        ("tool-results/skipped.txt", "Skipped {name}.\n", "name"),
        ("tool-results/unknown.txt", "No tool {tool}.\n", "tool"),
    ] {
        let path = software.join(file);
        let kept = std::fs::read_to_string(&path).expect("读得出");
        std::fs::write(&path, text).expect("写得进");
        let problems = Factory::load(&ResourceRoot::at(&resources)).expect_err("写坏了");
        assert_eq!(
            problems
                .iter()
                .map(|problem| (problem.code, problem.source, problem.file.as_str()))
                .collect::<Vec<_>>(),
            [(Code::BadFormat, Source::Factory, file)]
        );
        assert!(
            problems[0]
                .why
                .as_deref()
                .is_some_and(|why| why.contains(wanted)),
            "{problems:?}"
        );
        std::fs::remove_file(&path).expect("删得了");
        let problems = Factory::load(&ResourceRoot::at(&resources)).expect_err("不在");
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert_eq!(problems[0].file, file, "{problems:?}");
        std::fs::write(&path, kept).expect("写得进");
    }
    // 少要字段照收：答的那一句不带工具名也认得出。
    std::fs::write(software.join("tool-results/unknown.txt"), "No such tool.\n").expect("写得进");
    assert!(Factory::load(&ResourceRoot::at(&resources)).is_ok());
    clean(resources.parent().expect("有上一级"));
}
