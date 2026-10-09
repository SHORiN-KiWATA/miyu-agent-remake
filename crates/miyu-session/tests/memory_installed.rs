//! 记忆照包启用（施工 R-10，`docs/blueprint/memory.md` 第十一条）：核心照装没装人格记忆设 `Memory::set_installed`，开着的
//! 会话照这时的：没装的这一轮不交摘要、闲了不抽，装回来接着。真会话、真记忆日志；整理记忆的模型是一台假服务器。

use std::time::Duration;

use miyu_http::testkit::Server;
use miyu_kernel::block::{Block, Text};
use miyu_kernel::request::{Message, Request};
use miyu_session::testkit::{Play, Script};

use crate::support::extracting::*;
use crate::support::meaning::{catalog, chat, save};
use crate::support::*;

fn says(n: usize) -> Script {
    Script::new((0..n).map(|_| Play::Says("好。")))
}

/// 一次请求里记忆那一块（有几块交几块）。
fn summaries(request: &Request) -> Vec<String> {
    request
        .messages
        .iter()
        .flat_map(|message| match message {
            Message::User { blocks } => blocks.clone(),
            _ => Vec::new(),
        })
        .filter_map(|block| match block {
            Block::Text(Text { text }) if text.starts_with("<memories>") => Some(text),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn while_uninstalled_no_summary_is_given_and_after_reinstalling_it_is() {
    let home = Home::new();
    save(&home, "用户养了一只猫");
    let script = says(2);
    let handle = home
        .create_full(
            &script,
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    home.memory.set_installed(false);
    chat(&home, &handle, 1, "在吗").await;
    assert!(
        summaries(&script.requests()[0].1).is_empty(),
        "没装的这一轮不交"
    );
    home.memory.set_installed(true);
    chat(&home, &handle, 2, "再说一句").await;
    let given = summaries(&script.requests()[1].1);
    assert_eq!(given.len(), 1, "装回来下一轮交：{given:?}");
    assert!(
        given[0].contains("用户养了一只猫"),
        "以前记的都在：{given:?}"
    );
    stop(&handle).await;
}

#[tokio::test]
async fn while_uninstalled_nothing_is_extracted_and_after_reinstalling_it_is() {
    let server = Server::start(vec![every_turn("用户养了一只猫")]).await;
    let mut home = Home::new();
    organizer(&mut home, &server, "");
    let handle = home
        .create_full(
            &says(3),
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    home.memory.set_installed(false);
    chat(&home, &handle, 1, "我养了一只猫").await;
    chat(&home, &handle, 2, "它叫团子").await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(server.received().is_empty(), "没装的闲了不抽");
    home.memory.set_installed(true);
    chat(&home, &handle, 3, "三岁了").await;
    requested(&server, 1).await;
    stop(&handle).await;
}

#[tokio::test]
async fn an_alarm_set_before_uninstalling_does_not_extract() {
    let server = Server::start(vec![every_turn("不该有")]).await;
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
    chat(&home, &handle, 1, "一").await;
    chat(&home, &handle, 2, "二").await;
    // 闲了上了闹钟（100 毫秒），响之前卸掉。
    home.memory.set_installed(false);
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(server.received().is_empty(), "响的时候没装就不抽");
    stop(&handle).await;
}
