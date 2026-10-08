//! 包的工具变了，开着的会话下一个回合换上（施工 O-2 中，`docs/construction/O-2-提供者（中）.md`）：扩展登记以前就造好的会话，
//! 下一个回合工具面里有它的工具、调得通，记一条 `session.policy_changed`；关掉扩展，下一个回合没有了。登记缓存在磁盘上：重启
//! 以后扩展还没登记，核心照缓存先登记，载入的会话第一个回合对上，调到的暂时不可用；关掉的出目录，再开时照缓存先登记；关着的
//! 包重启时不读缓存。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::Body;
use miyu_kernel::origin::By;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::extensions::*;
use crate::support::*;

fn tools() -> Value {
    json!({"tools": [{"name": "echo_back", "description": "The echo_back tool.",
                      "input_schema": {"type": "object"}, "access": "read", "venues": ["local"]}]})
}

/// 等到 `path` 里记下的有 `n` 行，交回它们。
async fn lines(path: &std::path::Path, n: usize) -> Vec<String> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let got: Vec<String> = read(path).lines().map(str::to_string).collect();
        if got.len() >= n {
            return got;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "等不到 {n} 行：{got:?}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// 第 `n` 次请求的工具面。
fn face(script: &Script, n: usize) -> Vec<String> {
    script.requests()[n]
        .1
        .tools
        .iter()
        .map(|tool| tool.name.clone())
        .collect()
}

/// 内核换策略快照记的几条。
fn swaps(home: &Home, session: &str) -> usize {
    home.log(session)
        .iter()
        .filter(|event| {
            matches!(&event.body, Body::PolicyChanged(changed) if changed.policy.is_some())
                && event.by == By::Kernel
        })
        .count()
}

/// 最后一条工具结果的字。
fn last_result(home: &Home, session: &str) -> Option<String> {
    home.log(session)
        .into_iter()
        .filter_map(|event| match event.body {
            Body::ToolResult(result) => Some(
                result
                    .blocks
                    .iter()
                    .map(|block| match block {
                        Block::Text(Text { text }) => text.clone(),
                        _ => String::new(),
                    })
                    .collect::<String>(),
            ),
            _ => None,
        })
        .next_back()
}

#[tokio::test]
async fn a_session_made_before_provide_gets_the_tools_at_its_next_turn() {
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "bridge");
    install(
        &home,
        "bridge",
        &program.name(),
        "always",
        &steps(&[&step, "hello", &format!("ask:provide:{}", tools()), "serve"]),
    );
    let script = Script::new([
        Play::Says("好。"),
        Play::calls(&[("echo_back", "{}")]),
        Play::Says("嗯。"),
        Play::Says("没了。"),
    ]);
    let core = Arc::new(
        home.core_full(&script, Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("s1", &session, "在吗").await;
    home.until_turns(&session, 1).await;
    assert!(face(&script, 0).is_empty(), "扩展还没登记");

    core.start_extensions();
    let got = lines(&path, 3).await;
    let provided: Value = serde_json::from_str(&got[2]).unwrap();
    assert_eq!(provided["result"], json!({"tools": 1}), "{provided}");
    let cached = home.root.state().join("providers/bridge.json");
    let cached: Value = serde_json::from_str(&read(&cached)).expect("缓存是 JSON");
    assert_eq!(cached, tools(), "缓存照登记的原样");

    client.say("s2", &session, "调一下").await;
    home.until_turns(&session, 2).await;
    assert_eq!(face(&script, 1), ["echo_back"], "下一个回合换上");
    assert_eq!(swaps(&home, &session), 1);
    assert_eq!(
        last_result(&home, &session).as_deref(),
        Some("served echo_back")
    );

    let stopped = call(&mut client, "extension.disable", "bridge").await;
    assert!(stopped.get("error").is_none(), "{stopped}");
    until_state(&mut client, "bridge", |entry| entry["state"] == "off").await;
    client.say("s3", &session, "还有吗").await;
    home.until_turns(&session, 3).await;
    assert!(face(&script, 3).is_empty(), "关掉以后下一个回合拿掉");
    assert_eq!(swaps(&home, &session), 2);
    core.stop_extensions().await;
}

#[tokio::test]
async fn after_a_restart_the_cached_tools_are_there_before_the_extension_provides() {
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "bridge");
    install(
        &home,
        "bridge",
        &program.name(),
        "always",
        &steps(&[&step, "hello", &format!("ask:provide:{}", tools()), "serve"]),
    );
    let script = Script::new([Play::Says("好。")]);
    let first = Arc::new(
        home.core_full(&script, Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    let mut client = Client::connect(Arc::clone(&first));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("s1", &session, "在吗").await;
    home.until_turns(&session, 1).await;
    first.start_extensions();
    lines(&path, 3).await;
    first.stop_extensions().await;
    first.stop_sessions().await;
    drop(client);

    // 重启以后扩展握了手、不登记：工具照缓存在目录里，调到的暂时不可用。
    let (_, step) = record(&home, "bridge-again");
    install(
        &home,
        "bridge",
        &program.name(),
        "always",
        &steps(&[&step, "hello", "hang"]),
    );
    let script = Script::new([
        Play::calls(&[("echo_back", "{}")]),
        Play::Says("嗯。"),
        Play::Says("新的。"),
        Play::Says("关着。"),
        Play::Says("又开了。"),
    ]);
    let second = Arc::new(
        home.core_full(&script, Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    second.start_extensions();
    let mut client = Client::connect(Arc::clone(&second));
    client.hello().await;
    client.say("s2", &session, "调一下").await;
    home.until_turns(&session, 2).await;
    assert_eq!(face(&script, 0), ["echo_back"], "载入的第一个回合对上缓存");
    assert_eq!(
        last_result(&home, &session).as_deref(),
        Some("The tool \"echo_back\" is not available right now.\n")
    );
    let fresh = client.create("c2", "~").await;
    client.say("s3", &fresh, "你好").await;
    home.until_turns(&fresh, 1).await;
    assert_eq!(face(&script, 2), ["echo_back"], "新造的会话照缓存带上");

    // 关掉：工具出目录；再开：扩展还没登记，照缓存先登记。
    let stopped = call(&mut client, "extension.disable", "bridge").await;
    assert!(stopped.get("error").is_none(), "{stopped}");
    let off = client.create("c3", "~").await;
    client.say("s4", &off, "你好").await;
    home.until_turns(&off, 1).await;
    assert!(face(&script, 3).is_empty(), "关掉的出目录");
    let started = call(&mut client, "extension.enable", "bridge").await;
    assert!(started.get("error").is_none(), "{started}");
    let on = client.create("c4", "~").await;
    client.say("s5", &on, "你好").await;
    home.until_turns(&on, 1).await;
    assert_eq!(face(&script, 4), ["echo_back"], "开的时候照缓存先登记");

    // 关掉以后再重启：关着的包不读缓存。
    let stopped = call(&mut client, "extension.disable", "bridge").await;
    assert!(stopped.get("error").is_none(), "{stopped}");
    second.stop_extensions().await;
    second.stop_sessions().await;
    drop(client);
    let script = Script::new([Play::Says("好。")]);
    let third = Arc::new(
        home.core_full(&script, Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    third.start_extensions();
    let mut client = Client::connect(Arc::clone(&third));
    client.hello().await;
    let last = client.create("c5", "~").await;
    client.say("s6", &last, "你好").await;
    home.until_turns(&last, 1).await;
    assert!(face(&script, 0).is_empty(), "关着的包不读缓存");
}
