//! 现在就整理记忆（施工 R-7 补，`docs/blueprint/memory.md` 第七条第 9 款）：真会话、真记忆日志；整理记忆的模型是一台假服务器
//! （`org/m`），先交抽取的回答、再交合并的。
//!
//! 会话里的 `Handle::dream`：不等闲、答了一轮就抽，抽完不看会话数、间隔现在就合，交回几样数；没有要整理的交回 0、不发请求；
//! 不经会话的 `Keeper::dream` 照样合；这一间正在合的交回忙；没装人格记忆的用不了。

use std::sync::Arc;
use std::time::Duration;

use miyu_http::testkit::{Piece, Reply, Server};
use miyu_kernel::time::UtcOffset;
use miyu_session::testkit::{Play, Script};
use miyu_session::{Dreamed, Keeper, NotDreamed, Turn};

use crate::support::extracting::*;
use crate::support::meaning::{catalog, chat, persona, save};
use crate::support::*;

fn says(n: usize) -> Script {
    Script::new((0..n).map(|_| Play::Says("好。")))
}

/// 这一间的整理记忆的一方：照场地这时的配置、属主 alice、东九区。
async fn dream(home: &Home) -> Result<Dreamed, NotDreamed> {
    let keeper = Keeper::new(&home.memory, persona(), vec![alice()]);
    // 场地的配置是不变的那一种：最终值照它合出来的（带着 `memory.organizer`）。
    let source = Arc::clone(&*home.configs.borrow());
    let config = Arc::new(Turn::new(source.with_project("~"), source));
    keeper
        .dream(
            config,
            alice_account(),
            UtcOffset::from_minutes(540).expect("合写法"),
        )
        .await
}

/// 合并交回的一段，晚 `wait` 才回。
fn slow(json: serde_json::Value, wait: Duration) -> Reply {
    let mut reply = stream(&json.to_string());
    reply.body.insert(0, Piece::Wait(wait));
    reply
}

#[tokio::test]
async fn a_dream_in_a_session_extracts_now_and_merges_now() {
    let server = Server::start(vec![
        every_turn("用户养了一只猫"),
        stream(
            &serde_json::json!({
                "revised": [{"id": "m1", "text": "用户养了一只橘猫"}],
                "summary": "用户养了一只橘猫。"
            })
            .to_string(),
        ),
    ])
    .await;
    let mut home = Home::new();
    // 攒五轮才自己抽、五个会话才自己合：这里都不够，只有 dream 会做。
    organizer(&mut home, &server, "extract_turns = 5\n");
    let handle = home
        .create_full(
            &says(1),
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    chat(&home, &handle, 1, "我养了一只猫").await;
    let dreamed = handle.dream().await.expect("会话在跑").expect("整理了");
    assert_eq!(server.received().len(), 2, "抽了一次、合了一次");
    assert!(asked(&server, 0).starts_with("Find what is worth remembering"));
    assert!(asked(&server, 1).starts_with("Below are the memories"));
    assert_eq!(
        dreamed,
        Dreamed {
            given: 1,
            revised: 1,
            retired: 0,
            summary: true
        }
    );
    stop(&handle).await;
}

#[tokio::test]
async fn nothing_new_is_nothing_to_do_and_asks_nobody() {
    let server = Server::start(vec![
        stream(&serde_json::json!({"summary": "用户养猫。"}).to_string()),
        // 多备一个：要是又发了，看得出来。
        stream("{}"),
    ])
    .await;
    let mut home = Home::new();
    organizer(&mut home, &server, "");
    save(&home, "用户养了一只猫");
    let first = dream(&home).await.expect("整理了");
    assert_eq!(
        (first.given, first.summary),
        (1, true),
        "不经会话的照样合：{first:?}"
    );
    let second = dream(&home).await.expect("整理了");
    assert_eq!(
        second,
        Dreamed {
            given: 0,
            revised: 0,
            retired: 0,
            summary: false
        }
    );
    assert_eq!(server.received().len(), 1, "没有要整理的不发");
}

#[tokio::test]
async fn one_dream_at_a_time_in_a_room() {
    let server = Server::start(vec![
        slow(serde_json::json!({}), Duration::from_millis(500)),
        stream("{}"),
    ])
    .await;
    let mut home = Home::new();
    organizer(&mut home, &server, "");
    save(&home, "用户养了一只猫");
    let (a, b) = tokio::join!(dream(&home), async {
        tokio::time::sleep(Duration::from_millis(100)).await;
        dream(&home).await
    });
    assert!(a.is_ok(), "{a:?}");
    assert_eq!(b, Err(NotDreamed::Busy), "这一间正在合");
}

#[tokio::test]
async fn without_the_package_there_is_no_dream() {
    let server = Server::start(vec![stream("{}")]).await;
    let mut home = Home::new();
    organizer(&mut home, &server, "");
    save(&home, "用户养了一只猫");
    home.memory.set_installed(false);
    assert_eq!(dream(&home).await, Err(NotDreamed::Off));
    assert!(server.received().is_empty());
}
