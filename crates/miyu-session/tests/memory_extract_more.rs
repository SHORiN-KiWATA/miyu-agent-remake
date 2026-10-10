//! 抽取不丢、不卡（施工 R-6 上，`docs/blueprint/memory.md` 第六条）：两次抽取之间压缩过，检查点以前的几轮照样抽到；重开会话
//! 照抽到的地方接着；一段放不下的分两次抽完；同一段连着三次抽不成的放过、往前走。
//!
//! 整理记忆的模型是假服务器，会话的模型照剧本回（`crate::support::extracting`）。

use std::time::Duration;

use miyu_http::testkit::{Reply, Server};
use miyu_kernel::session::Command;
use miyu_session::testkit::{Play, Script};

use crate::support::extracting::*;
use crate::support::meaning::{catalog, chat};
use crate::support::*;

fn says(n: usize) -> Script {
    Script::new((0..n).map(|_| Play::Says("好。")))
}

/// 等会话 `session` 抽到的地方过了 `past`（最多 10 秒）。
async fn marked_past(home: &Home, session: &miyu_kernel::id::SessionId, past: Option<u64>) {
    for _ in 0..200 {
        if mark(home, session).map(|seq| seq.get()) > past {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("十秒没往前走：{:?}", mark(home, session));
}

#[tokio::test]
async fn turns_before_a_compaction_are_still_extracted() {
    let server = Server::start(vec![every_turn("用户养了一只猫")]).await;
    let mut home = Home::new();
    organizer(&mut home, &server, "extract_turns = 3\n");
    // 窗口大到不会自动压；第一轮够长，手动压缩压得掉它（同 `manual_compaction_log.rs`）。
    let script = Script::new([
        Play::Says("好。"),
        Play::Says("好。"),
        Play::Says("<summary>用户说了很长的一段。</summary>"),
        Play::Says("好。"),
    ])
    .window(1_000_000);
    let handle = home
        .create_full(
            &script,
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    chat(
        &home,
        &handle,
        1,
        &format!("我养了一只猫。{}", "x".repeat(70_000)),
    )
    .await;
    chat(&home, &handle, 2, "二").await;
    let mut pushes = watch(&handle).await;
    let compact = Command::Compact { instructions: None };
    ask(&handle, "cmd-compact", compact)
        .await
        .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    assert!(server.received().is_empty(), "压缩以前不够三轮");
    chat(&home, &handle, 3, "三").await;
    requested(&server, 1).await;
    let text = asked(&server, 0);
    assert!(text.contains("我养了一只猫"), "检查点以前的那一轮照样在");
    assert!(text.contains("[... excerpted ...]"), "那一轮太长，截了中间");
    stop(&handle).await;
}

#[tokio::test]
async fn a_reopened_session_goes_on_from_where_it_was_extracted() {
    let server = Server::start(vec![every_turn("第一次"), every_turn("第二次")]).await;
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
    let session = handle.id().clone();
    chat(&home, &handle, 1, "一").await;
    chat(&home, &handle, 2, "二").await;
    marked_past(&home, &session, None).await;
    let first = mark(&home, &session).map(|seq| seq.get());
    stop(&handle).await;
    let handle = home.load_with(&session, &says(2), &catalog(&home)).await;
    chat(&home, &handle, 3, "三").await;
    chat(&home, &handle, 4, "四").await;
    requested(&server, 2).await;
    let text = asked(&server, 1);
    assert!(
        text.contains("User: 三") && text.contains("User: 四"),
        "{text}"
    );
    assert!(
        !text.contains("User: 一") && !text.contains("User: 二"),
        "抽过的不重抽"
    );
    marked_past(&home, &session, first).await;
    assert_eq!(extracted(&home).len(), 4, "两次各记两轮");
    stop(&handle).await;
}

#[tokio::test]
async fn what_does_not_fit_is_extracted_right_after() {
    let server = Server::start(vec![every_turn("前两轮"), every_turn("后两轮")]).await;
    let mut home = Home::new();
    organizer(&mut home, &server, "");
    let handle = home
        .create_full(
            &says(4),
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    let long = |n: &str| format!("{n}{}", "长".repeat(4_000));
    // 一轮约 12 KiB：两轮放得下、三轮放不下。闹钟在第二轮以后先响的，也是先抽前两轮、再抽后两轮。
    for (turn, said) in ["一", "二", "三", "四"].into_iter().enumerate() {
        chat(&home, &handle, turn + 1, &long(said)).await;
    }
    requested(&server, 2).await;
    let (first, second) = (asked(&server, 0), asked(&server, 1));
    assert!(
        first.contains("User: 一") && first.contains("User: 二") && !first.contains("User: 三")
    );
    assert!(
        second.contains("User: 三") && second.contains("User: 四") && !second.contains("User: 二")
    );
    stop(&handle).await;
}

#[tokio::test]
async fn three_failures_on_the_same_slice_let_it_go() {
    let failed = || Reply::error(500, &[], r#"{"error":{"message":"down"}}"#);
    let server = Server::start(vec![failed(), failed(), failed(), every_turn("后来的")]).await;
    let mut home = Home::new();
    organizer(&mut home, &server, "");
    let handle = home
        .create_full(
            &says(6),
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    let session = handle.id().clone();
    chat(&home, &handle, 1, "一").await;
    chat(&home, &handle, 2, "二").await;
    requested(&server, 1).await;
    chat(&home, &handle, 3, "三").await;
    requested(&server, 2).await;
    assert_eq!(mark(&home, &session), None, "失败的不往前挪");
    chat(&home, &handle, 4, "四").await;
    requested(&server, 3).await;
    marked_past(&home, &session, None).await;
    assert!(extracted(&home).is_empty(), "放过的一条都没记");
    chat(&home, &handle, 5, "五").await;
    chat(&home, &handle, 6, "六").await;
    requested(&server, 4).await;
    let text = asked(&server, 3);
    assert!(
        text.contains("User: 五") && !text.contains("User: 四"),
        "{text}"
    );
    stop(&handle).await;
}
