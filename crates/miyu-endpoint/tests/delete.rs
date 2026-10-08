//! 删除会话（施工 3-8 三补，`docs/blueprint/protocol.md` 的 `session.delete`）：空闲的会话挪进回收处
//! `home/<账号>/trash/sessions/<会话编号>/`，写下 `deleted_at`，回应 `{}`；删了的列不出来，再发命令是没有这个会话。有回合在
//! 进行的拒绝、什么都不动。没在跑的会话不载入就删：被重启打断的那一轮不接着干。子会话的几种在 `delete_children.rs`。

use serde_json::json;

use miyu_kernel::time::Timestamp;
use miyu_session::testkit::{Play, Script};

use crate::support::deleting::*;
use crate::support::*;

/// 这个时刻：`deleted_at` 照它比。
fn now() -> Timestamp {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("1970 年以后")
        .as_millis();
    Timestamp::from_unix_millis(i64::try_from(millis).expect("放得下")).expect("在范围里")
}

#[tokio::test]
async fn an_idle_session_moves_to_the_trash_and_is_gone() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。")]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let work = home.work.to_string_lossy().into_owned();
    let (kept, session) = (
        client.create("c1", &work).await,
        client.create("c2", &work).await,
    );
    client.subscribe("s1", &session).await;
    client.say("m1", &session, "在吗").await;
    home.until_turns(&session, 1).await;
    let log = home.log(&session);
    let before = now();
    let (_, reply) = delete(&mut client, "d1", &session).await;
    let after = now();
    assert_eq!(reply["result"], json!({}), "{reply}");

    // 整个目录挪进回收处，日志一个字节不变，多一个 `deleted_at`。
    assert!(!in_place(&home, &session), "原处没了");
    assert_eq!(trashed_log(&home, &session), log);
    let written = std::fs::read_to_string(trashed(&home, &session).join("deleted_at"))
        .expect("写了 deleted_at");
    assert!(written.ends_with('\n'), "{written:?}");
    let at = Timestamp::parse(written.trim_end()).expect("写的是事件的时刻写法");
    assert!(before <= at && at <= after, "删的时刻：{written}");

    // 列不出来；再发命令、订阅、改名、再删：没有这个会话。另一个会话照旧。
    assert_eq!(listed(&mut client, "l1").await, [kept.as_str()]);
    for (id, method, params) in [
        (
            "x1",
            "session.send",
            json!({"session": session, "text": "还在吗"}),
        ),
        (
            "x2",
            "subscribe",
            json!({"session": session, "stream": "events"}),
        ),
        (
            "x3",
            "session.set_meta",
            json!({"session": session, "title": "旧的"}),
        ),
        ("x4", "session.delete", json!({"session": session})),
        (
            "x5",
            "session.interrupt",
            json!({"session": session, "queued": "return"}),
        ),
    ] {
        let reply = client.call(id, method, params).await;
        assert_eq!(reason(&reply), Some("session_not_found"), "{id}: {reply}");
    }
    assert!(in_place(&home, &kept));
    // 重发造它的那一条：不再交回删了的，另造一个。
    let again = client.create("c2", &work).await;
    assert_ne!(again, session);
    assert!(in_place(&home, &again));
}

#[tokio::test]
async fn a_running_turn_is_refused_and_nothing_moves() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([Play::Holds])));
    client.hello().await;
    let work = home.work.to_string_lossy().into_owned();
    let session = client.create("c1", &work).await;
    client.say("m1", &session, "在吗").await;
    let (_, reply) = delete(&mut client, "d1", &session).await;
    assert_eq!(reason(&reply), Some("turn_running"), "{reply}");
    assert_eq!(
        reply["error"]["message"],
        json!("有回合在进行：先打断，或者等它做完。")
    );
    assert!(in_place(&home, &session));
    assert!(!trashed(&home, &session).exists());
    // 会话照常：打断得了，打断以后删得掉。
    let reply = client
        .call(
            "i1",
            "session.interrupt",
            json!({"session": session, "queued": "return"}),
        )
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
    let (_, reply) = delete(&mut client, "d2", &session).await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    assert!(trashed(&home, &session).exists());
}

#[tokio::test]
async fn a_session_not_running_is_moved_without_being_loaded() {
    let home = Home::new();
    let script = Script::new([Play::Holds]);
    let first = home.core(&script);
    let mut client = Client::connect(first.clone());
    client.hello().await;
    let work = home.work.to_string_lossy().into_owned();
    let session = client.create("c1", &work).await;
    client.say("m1", &session, "在吗").await;
    // 有计划地重启：跑到一半的那一轮记成重启了，载入的话会接着干。
    first.stop_sessions().await;
    let log = home.log(&session);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let (_, reply) = delete(&mut client, "d1", &session).await;
    assert_eq!(reply["result"], json!({}), "没载入，也就没接着干：{reply}");
    assert_eq!(trashed_log(&home, &session), log, "一条都没多");
    assert!(listed(&mut client, "l1").await.is_empty());
}

#[tokio::test]
async fn wrong_params_and_unknown_sessions_are_refused() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    for (id, params) in [
        ("b1", json!({})),
        ("b2", json!({"session": "not-a-session"})),
        ("b3", json!({"session": 7})),
    ] {
        let reply = client.call(id, "session.delete", params).await;
        assert_eq!(reason(&reply), Some("bad_params"), "{id}: {reply}");
    }
    let reply = client
        .call(
            "n1",
            "session.delete",
            json!({"session": "01900000-0000-7000-8000-000000000000"}),
        )
        .await;
    assert_eq!(reason(&reply), Some("session_not_found"), "{reply}");
}
