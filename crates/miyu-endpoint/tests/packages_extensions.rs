//! 装卸当场生效的扩展那一半（施工 F-5 下，`docs/blueprint/packages.md`「装卸」，设计 30 第九节）：真核心、真的测试扩展走一遍。
//! 装上一个开着就拉起的扩展，当场拉起、它登记的工具进目录；卸掉，当场停下、`extension.status` 不列它，它的工具出目录、
//! 用过它的会话照旧留着、调到时报「已卸载」；升级了的停下再拉起，装别的包时清单没变的不动。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::Body;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::extensions::*;
use crate::support::*;

/// 测试扩展登记的一件工具。
fn tools() -> Value {
    json!({"tools": [{"name": "echo_back", "description": "The echo_back tool.",
                      "input_schema": {"type": "object"}, "access": "read", "venues": ["local"]}]})
}

/// 等到 `path` 里记下的有 `n` 行。
async fn lines(path: &std::path::Path, n: usize) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while read(path).lines().count() < n {
        assert!(
            tokio::time::Instant::now() < deadline,
            "等不到 {n} 行：{}",
            read(path)
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

/// 会话最后一条工具结果的字。
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
async fn an_installed_extension_starts_and_a_removed_one_stops() {
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "xbridge");
    // 先装到家目录里拿到清单的写法，再挪到工作目录当要装的那一份。
    install(
        &home,
        "xbridge",
        &program.name(),
        "always",
        &steps(&[
            &step,
            "hello",
            &format!("ask:provide:{}", tools()),
            "serve",
            "err:stopped",
        ]),
    );
    let written = home.root.path().join("home/alice/packages/xbridge.toml");
    let source = home.work.join("xbridge.toml");
    std::fs::rename(&written, &source).expect("挪得动");
    let script = Script::new([
        Play::calls(&[("echo_back", "{}")]),
        Play::Says("嗯。"),
        Play::calls(&[("echo_back", "{}")]),
        Play::Says("没了。"),
    ]);
    let core = Arc::new(
        home.core_full(&script, Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    core.start_extensions();
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let reply = client
        .call("i1", "package.install", json!({"path": source}))
        .await;
    assert_eq!(reply["result"]["package"], "xbridge", "{reply}");
    until_state(&mut client, "xbridge", |entry| entry["state"] == "running").await;
    lines(&path, 3).await;

    let session = client.create("c1", "~").await;
    client.say("s1", &session, "调一下").await;
    home.until_turns(&session, 1).await;
    assert_eq!(face(&script, 0), ["echo_back"], "装上就有它的工具");
    assert_eq!(
        last_result(&home, &session).as_deref(),
        Some("served echo_back")
    );

    let reply = client
        .call("r1", "package.remove", json!({"package": "xbridge"}))
        .await;
    assert_eq!(reply["result"]["removed"], true, "{reply}");
    until("卸掉的停下", || {
        stderr(&home, "xbridge").contains("stopped")
    })
    .await;
    let listed = client.call("s", "extension.status", json!({})).await;
    assert!(
        listed["result"]["extensions"]
            .as_array()
            .expect("有")
            .iter()
            .all(|one| one["package"] != "xbridge"),
        "卸掉的不列：{listed}"
    );
    client.say("s2", &session, "再调一下").await;
    home.until_turns(&session, 2).await;
    assert_eq!(face(&script, 2), ["echo_back"], "用过它的会话照旧留着");
    assert_eq!(
        last_result(&home, &session).as_deref(),
        Some("The tool \"echo_back\" was uninstalled.\n")
    );
    core.stop_extensions().await;
}

/// 升级一个在跑的扩展（施工 F-5 下）：清单变了，先停下再照开关拉起，新起来的那一个又记一遍。
#[tokio::test]
async fn an_upgraded_extension_restarts() {
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "xup");
    let written = home.root.path().join("home/alice/packages/xup.toml");
    let source = home.work.join("xup.toml");
    install(
        &home,
        "xup",
        &program.name(),
        "always",
        &steps(&[&step, "hello", "serve"]),
    );
    std::fs::rename(&written, &source).expect("挪得动");
    let core = Arc::new(
        home.core_full(&Script::new([]), Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    core.start_extensions();
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let reply = client
        .call("i1", "package.install", json!({"path": source}))
        .await;
    assert_eq!(reply["result"]["package"], "xup", "{reply}");
    lines(&path, 2).await;
    let running = until_state(&mut client, "xup", |entry| entry["state"] == "running").await;
    install(&home, "xoff", &program.name(), "manual", &steps(&["serve"]));
    let other = home.work.join("xoff.toml");
    std::fs::rename(
        home.root.path().join("home/alice/packages/xoff.toml"),
        &other,
    )
    .expect("挪得动");
    let reply = client
        .call("i0", "package.install", json!({"path": other}))
        .await;
    assert_eq!(reply["result"]["package"], "xoff", "{reply}");
    let same = status(&mut client, "xup").await;
    assert_eq!(same["pid"], running["pid"], "清单没变的不重起：{same}");
    assert_eq!(read(&path).lines().count(), 2, "{}", read(&path));
    install(
        &home,
        "xup",
        &program.name(),
        "always",
        &steps(&[&step, "hello", "err:v2", "serve"]),
    );
    std::fs::rename(&written, &source).expect("挪得动");
    let reply = client
        .call("i2", "package.install", json!({"path": source}))
        .await;
    assert_eq!(reply["result"]["package"], "xup", "{reply}");
    lines(&path, 4).await;
    until_state(&mut client, "xup", |entry| entry["state"] == "running").await;
    core.stop_extensions().await;
}
