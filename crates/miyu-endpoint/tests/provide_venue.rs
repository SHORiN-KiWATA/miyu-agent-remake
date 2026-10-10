//! 登记成 `venue` 的工具（施工 O-31 前，`providers.md`「`provide`」第 2 条、`session/guard.md` 第四条）：`provide` 收；本机的
//! 会话调到它，权限策略当场拒绝，写给她出厂的那一句，扩展收不到 `tool.call`。场所会话里放行见 `miyu-session` 的
//! `guard_venue.rs`。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Body, ToolStatus};
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::extensions::*;
use crate::support::*;

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

#[tokio::test]
async fn a_venue_tool_is_provided_and_refused_in_a_local_session() {
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "bridge");
    let tools = json!({"tools": [{"name": "kick", "description": "Kick someone.",
        "input_schema": {"type": "object"}, "access": "venue", "venues": ["local", "group"]}]});
    install(
        &home,
        "bridge",
        &program.name(),
        "always",
        &steps(&[&step, "hello", &format!("ask:provide:{tools}"), "serve"]),
    );
    let script = Script::new([Play::calls(&[("kick", "{}")]), Play::Says("好。")]);
    let core = Arc::new(
        home.core_full(&script, Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    core.start_extensions();
    let got = lines(&path, 3).await;
    let provided: Value = serde_json::from_str(&got[2]).unwrap();
    assert_eq!(provided["result"], json!({"tools": 1}), "{provided}");

    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("s1", &session, "踢一下").await;
    home.until_turns(&session, 1).await;
    let (text, status) = home
        .log(&session)
        .into_iter()
        .find_map(|event| match event.body {
            Body::ToolResult(result) => {
                let text: String = result
                    .blocks
                    .iter()
                    .map(|block| match block {
                        Block::Text(Text { text }) => text.clone(),
                        _ => String::new(),
                    })
                    .collect();
                Some((text, result.status))
            }
            _ => None,
        })
        .expect("有一条工具结果");
    let shipped =
        std::fs::read_to_string(default_resources().join("core/permissions/not-in-venue.txt"))
            .expect("读得出");
    assert_eq!((text, status), (shipped, ToolStatus::Denied));
    assert!(
        !read(&path).contains(r#""method":"tool.call""#),
        "扩展收不到：{}",
        read(&path)
    );
}
