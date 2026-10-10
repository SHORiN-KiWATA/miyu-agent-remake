//! 抽取（施工 R-6 上，`docs/blueprint/memory.md` 第六条）：会话闲下来以后，把这一段发给整理记忆的模型（假服务器上的
//! `org/m`，经一次性入口），交回的记成记忆。会话的模型照剧本回。
//!
//! 两轮以后闲了就抽：一条 user，指令在前，后面一轮一块；没有 system；记下的 `by` 是记忆模块、出处是那一轮、听众是属主；
//! 记下抽到了哪。只有一轮的不抽；她这一段调过 `remember` 的整段跳过、不发；人说的 key 发出去以前遮掉。

use std::time::Duration;

use miyu_http::testkit::Server;
use miyu_kernel::origin::By;
use miyu_session::testkit::{Play, Script};

use crate::support::extracting::*;
use crate::support::meaning::{catalog, chat};
use crate::support::*;

fn says(n: usize) -> Script {
    Script::new((0..n).map(|_| Play::Says("好。")))
}

#[tokio::test]
async fn two_turns_and_a_pause_are_extracted_into_memories() {
    let server = Server::start(vec![every_turn("用户养了一只猫，叫团子")]).await;
    let mut home = Home::new();
    organizer(&mut home, &server, "");
    let handle = home
        .create_full(
            &says(2),
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    chat(&home, &handle, 1, "我养了一只猫").await;
    chat(&home, &handle, 2, "它叫团子").await;
    requested(&server, 1).await;
    let turns = ended_turns(&home, handle.id());
    let text = asked(&server, 0);
    assert!(text.starts_with("Find what is worth remembering"), "{text}");
    for turn in &turns {
        assert!(
            text.contains(&format!("<turn number=\"{}\"", turn.started().get())),
            "{text}"
        );
    }
    assert!(text.contains("User: 我养了一只猫") && text.contains("Assistant: 好。"));
    let body: serde_json::Value = serde_json::from_slice(&server.received()[0].body).expect("JSON");
    assert_eq!(body["model"], "m");
    assert_eq!(
        body["messages"].as_array().map(Vec::len),
        Some(1),
        "没有 system"
    );
    // 记下：每一轮一条（交回里别的轮不在这一段里，丢掉），记忆模块记的、出处是那一轮、听众是属主。
    for _ in 0..100 {
        if mark(&home, handle.id()).is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let found = extracted(&home);
    assert_eq!(found.len(), turns.len(), "{found:?}");
    for (entry, turn) in found.iter().zip(&turns) {
        assert_eq!(entry.text, "用户养了一只猫，叫团子");
        assert_eq!(entry.sources[0].session, *handle.id());
        assert_eq!(entry.sources[0].turn, *turn);
        assert_eq!(entry.audience, vec![alice()]);
        assert!(matches!(&entry.by, By::Module(module) if module.id.as_str() == "memory"));
    }
    assert!(mark(&home, handle.id()).is_some(), "记下了抽到哪");
    stop(&handle).await;
}

#[tokio::test]
async fn one_turn_is_not_enough_and_a_new_turn_resets_the_pause() {
    let server = Server::start(vec![every_turn("不该有")]).await;
    let mut home = Home::new();
    organizer(&mut home, &server, "extract_turns = 3\n");
    let handle = home
        .create_full(
            &says(3),
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    chat(&home, &handle, 1, "一").await;
    chat(&home, &handle, 2, "二").await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(server.received().is_empty(), "两轮不够三轮");
    chat(&home, &handle, 3, "三").await;
    requested(&server, 1).await;
    stop(&handle).await;
}

#[tokio::test]
async fn a_slice_where_she_remembered_is_skipped_without_asking() {
    let server = Server::start(vec![every_turn("不该有")]).await;
    let mut home = Home::new();
    organizer(&mut home, &server, "");
    let remember = serde_json::json!({"class": "user", "text": "用户养了一只猫"}).to_string();
    let script = Script::new([
        Play::calls(&[("remember", &remember)]),
        Play::Says("记住了。"),
        Play::Says("好。"),
    ]);
    let handle = home
        .create_full(
            &script,
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    chat(&home, &handle, 1, "记住我养了一只猫").await;
    chat(&home, &handle, 2, "好的").await;
    for _ in 0..100 {
        if mark(&home, handle.id()).is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(mark(&home, handle.id()).is_some(), "跳过的也记抽到了哪");
    assert!(server.received().is_empty(), "不发");
    assert!(extracted(&home).is_empty());
    stop(&handle).await;
}

#[tokio::test]
async fn keys_said_in_the_conversation_are_redacted_before_sending() {
    let server = Server::start(vec![every_turn("用户的 key 是 sk-org-0123456789abcdef")]).await;
    let mut home = Home::new();
    organizer(&mut home, &server, "");
    let handle = home
        .create_full(
            &says(2),
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    chat(&home, &handle, 1, "我的 key 是 sk-org-0123456789abcdef").await;
    chat(&home, &handle, 2, "还有一个 ghp_ABCDEFGHIJKLMNOPQRSTUV").await;
    requested(&server, 1).await;
    let text = asked(&server, 0);
    assert!(
        !text.contains("sk-org-0123456789abcdef"),
        "配置里引用的密钥的原文"
    );
    assert!(!text.contains("ghp_ABCDEFGHIJKLMNOPQRSTUV"), "常见的写法");
    assert_eq!(text.matches("[REDACTED]").count(), 2, "{text}");
    for _ in 0..100 {
        if mark(&home, handle.id()).is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let found = extracted(&home);
    assert!(!found.is_empty());
    assert!(
        found
            .iter()
            .all(|entry| entry.text == "用户的 key 是 [REDACTED]"),
        "交回来的再遮一遍：{found:?}"
    );
    stop(&handle).await;
}
