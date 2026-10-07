//! 改了预设的文件，下一个回合换上（施工 P-2 下，`docs/blueprint/presets.md`「改了文件」）：真核心走一遍。关掉一件工具，
//! 下一轮的请求里就没有它，日志里一条内核记的、带 `policy` 的 `session.policy_changed`；没改的不记；写错了的照旧；重启以后
//! 照换上的那一份。

mod support;

use std::sync::Arc;

use serde_json::json;

use miyu_endpoint::Core;
use miyu_kernel::event::Body;
use miyu_kernel::origin::By;
use miyu_kernel::request::Request;
use miyu_kernel::tool::Access;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use support::venues::configured_core;
use support::*;

/// 基础系统两件。
fn catalog() -> Catalog {
    let fake = |name: &str| -> Arc<dyn Tool> { Fake::new(name, Access::Read, Act::Echo) };
    Catalog::in_packages([("basesystem", vec![fake("read"), fake("shell")])]).expect("合写法")
}

fn configured(home: &Home, script: &Script) -> Arc<Core> {
    configured_core(home, script, catalog())
}

/// 家目录里的预设 `p`。
fn preset(home: &Home, text: &str) {
    home.write("home/alice/presets/p.toml", text);
}

/// 日志里内核记的换快照那几条。
fn swaps(home: &Home, session: &str) -> usize {
    home.log(session)
        .iter()
        .filter(|event| {
            event.by == By::Kernel
                && matches!(&event.body, Body::PolicyChanged(changed) if changed.policy.is_some())
        })
        .count()
}

fn tool_names(request: &Request) -> Vec<String> {
    request.tools.iter().map(|tool| tool.name.clone()).collect()
}

#[tokio::test]
async fn an_edited_preset_takes_over_on_the_next_turn() {
    let home = Home::new();
    preset(&home, "[preset]\nname = { en = \"P\" }\n");
    let script = Script::new([
        Play::Says("一。"),
        Play::Says("二。"),
        Play::Says("三。"),
        Play::Says("四。"),
    ]);
    let mut client = Client::connect(configured(&home, &script));
    client.hello().await;
    let reply = client
        .call("c1", "session.create", json!({"cwd": "~", "preset": "p"}))
        .await;
    let session = reply["result"]["session"]
        .as_str()
        .expect("造出来了")
        .to_string();
    client.say("s1", &session, "hi").await;
    home.until_turns(&session, 1).await;
    client.say("s2", &session, "hi").await;
    home.until_turns(&session, 2).await;
    assert_eq!(swaps(&home, &session), 0, "没改的不换");
    preset(
        &home,
        "[preset]\nname = { en = \"P\" }\n\n[tools]\nshell = false\n",
    );
    client.say("s3", &session, "hi").await;
    home.until_turns(&session, 3).await;
    assert_eq!(swaps(&home, &session), 1, "改了换一次");
    // 写错了的照旧用换上的那一份。
    preset(&home, "[tools]\nshell = true\n");
    client.say("s4", &session, "hi").await;
    home.until_turns(&session, 4).await;
    assert_eq!(swaps(&home, &session), 1, "写错了的不换");
    let requests: Vec<Vec<String>> = script
        .requests()
        .iter()
        .map(|(_, request)| tool_names(request))
        .collect();
    assert_eq!(
        requests,
        [
            vec!["read", "shell"],
            vec!["read", "shell"],
            vec!["read"],
            vec!["read"]
        ]
    );
    // 重启以后照换上的那一份。
    preset(
        &home,
        "[preset]\nname = { en = \"P\" }\n\n[tools]\nshell = false\n",
    );
    let script = Script::new([Play::Says("五。"), Play::Says("六。")]);
    let mut client = Client::connect(configured(&home, &script));
    client.hello().await;
    client.say("s5", &session, "hi").await;
    home.until_turns(&session, 5).await;
    assert_eq!(swaps(&home, &session), 1, "载入以后没改的不再换");
    // 载入的会话照样看预设：改回来，下一轮又换。
    preset(&home, "[preset]\nname = { en = \"P\" }\n");
    client.say("s6", &session, "hi").await;
    home.until_turns(&session, 6).await;
    assert_eq!(swaps(&home, &session), 2, "载入的会话改了也换");
    let requests: Vec<Vec<String>> = script
        .requests()
        .iter()
        .map(|(_, request)| tool_names(request))
        .collect();
    assert_eq!(requests, [vec!["read"], vec!["read", "shell"]]);
}
