//! 改了人格的文件，下一个回合换上（施工 P-1 再补，`docs/blueprint/kernel/session.md`「换策略快照」）：真核心走一遍。改了人设
//! 下一轮的 system 就是新的，日志里一条内核记的、带 `policy` 的 `session.policy_changed`；新人格带了角色扮演提示的这一轮就
//! 有；没改的不记；写错了的照旧用原来的；重启以后照换上的那一份。

mod support;

use std::sync::Arc;

use serde_json::json;

use miyu_endpoint::Core;
use miyu_kernel::event::Body;
use miyu_kernel::origin::By;
use miyu_kernel::request::Request;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use support::venues::configured_core;
use support::*;

/// 管理员（测试里是 alice）家目录里 Miyu 的一个文件。
fn mine(home: &Home, file: &str, text: &str) {
    home.write(&format!("home/alice/personas/miyu/{file}"), text);
}

fn configured(home: &Home, script: &Script) -> Arc<Core> {
    configured_core(home, script, Catalog::default())
}

/// 用 Miyu 开一个会话，交回它的编号。
async fn miyu(client: &mut Client) -> String {
    let reply = client
        .call(
            "c1",
            "session.create",
            json!({"cwd": "~", "persona": "miyu"}),
        )
        .await;
    reply["result"]["session"]
        .as_str()
        .expect("造出来了")
        .to_string()
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

/// 一次请求的最后一条人这边的消息里，有没有角色扮演提示。
fn reminded(request: &Request) -> bool {
    serde_json::to_string(&request.messages)
        .expect("请求写得成 JSON")
        .contains("persona-reminder")
}

#[tokio::test]
async fn an_edited_persona_takes_over_on_the_next_turn() {
    let home = Home::new();
    mine(&home, "prompts/persona.md", "You are Miyu.\n");
    let script = Script::new([Play::Says("嗯。"), Play::Says("好。"), Play::Says("在。")]);
    let mut client = Client::connect(configured(&home, &script));
    client.hello().await;
    let session = miyu(&mut client).await;
    client.say("s1", &session, "hi").await;
    home.until_turns(&session, 1).await;
    mine(
        &home,
        "prompts/persona.md",
        "You are Miyu, and you speak softly.\n",
    );
    mine(&home, "prompts/reminders.md", "Stay soft.\n");
    client.say("s2", &session, "再说").await;
    home.until_turns(&session, 2).await;
    client.say("s3", &session, "还在吗").await;
    home.until_turns(&session, 3).await;
    let requests = script.requests();
    assert!(requests[0].1.system.starts_with("You are Miyu.\n\n"));
    assert!(
        requests[1]
            .1
            .system
            .starts_with("You are Miyu, and you speak softly."),
        "{}",
        requests[1].1.system
    );
    assert!(
        requests[1].1.system.ends_with("</style-lock>"),
        "新人格带提示，接风格锁"
    );
    assert!(!reminded(&requests[0].1));
    assert!(reminded(&requests[1].1), "换上的这一轮就有提示");
    assert_eq!(swaps(&home, &session), 1, "第三轮没改，不再换");
    assert_eq!(requests[2].1.system, requests[1].1.system);
}

#[tokio::test]
async fn a_broken_persona_keeps_the_old_one() {
    let home = Home::new();
    mine(&home, "prompts/persona.md", "You are Miyu.\n");
    let script = Script::new([Play::Says("嗯。"), Play::Says("好。")]);
    let mut client = Client::connect(configured(&home, &script));
    client.hello().await;
    let session = miyu(&mut client).await;
    client.say("s1", &session, "hi").await;
    home.until_turns(&session, 1).await;
    mine(&home, "prompts/persona.md", "You are someone else.\n");
    mine(&home, "prompts/examples.md", "user: a\n");
    client.say("s2", &session, "再说").await;
    home.until_turns(&session, 2).await;
    let requests = script.requests();
    assert_eq!(requests[1].1.system, requests[0].1.system, "写错了照旧");
    assert_eq!(swaps(&home, &session), 0);
}

#[tokio::test]
async fn after_a_restart_the_swapped_snapshot_is_loaded() {
    let home = Home::new();
    mine(&home, "prompts/persona.md", "You are Miyu.\n");
    let script = Script::new([Play::Says("嗯。"), Play::Says("好。"), Play::Says("在。")]);
    let first = configured(&home, &script);
    let mut client = Client::connect(first.clone());
    client.hello().await;
    let session = miyu(&mut client).await;
    client.say("s1", &session, "hi").await;
    home.until_turns(&session, 1).await;
    mine(
        &home,
        "prompts/persona.md",
        "You are Miyu, and you speak softly.\n",
    );
    client.say("s2", &session, "再说").await;
    home.until_turns(&session, 2).await;
    first.stop_sessions().await;
    drop(client);
    let mut client = Client::connect(configured(&home, &script));
    client.hello().await;
    client.say("s3", &session, "还在吗").await;
    home.until_turns(&session, 3).await;
    let requests = script.requests();
    assert!(
        requests[2]
            .1
            .system
            .starts_with("You are Miyu, and you speak softly.")
    );
    assert_eq!(requests[2].1.system, requests[1].1.system);
    assert_eq!(swaps(&home, &session), 1, "载入的就是换上的那一份，不再换");
}
