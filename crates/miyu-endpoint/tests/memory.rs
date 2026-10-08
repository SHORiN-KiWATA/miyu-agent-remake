//! 回合索引接在核心上（施工 R-2 上，`docs/blueprint/memory.md`「怎么走」第一条第 4、7 款）：真核心造的主会话说过的一轮进
//! 软件工程师的回合库；删会话以后拿掉。

use serde_json::json;

use miyu_session::testkit::{Play, Script};

use crate::support::*;

/// 另开一个只读的连接，在软件工程师的回合库里搜 `words`，交回键。回合库是核心用到才建的：还没有的是什么都没找到。不能
/// 用 `RecallIndex::open` 去开：它会建库、碰到核心正在建的那一半当成坏了删掉重建（一个库只该有一个连接写）。
fn found(home: &Home, words: &str) -> Vec<String> {
    let path = home
        .root
        .index(&alice())
        .join("recall")
        .join("turns-engineer.db");
    let Ok(db) =
        rusqlite::Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
    else {
        return Vec::new();
    };
    let query = miyu_recall::query(words).expect("切得出词");
    let Ok(mut select) = db.prepare(
        "SELECT items.key FROM terms JOIN items ON items.id = terms.rowid WHERE terms MATCH ?1",
    ) else {
        return Vec::new();
    };
    select
        .query_map([query], |row| row.get::<_, String>(0))
        .expect("查得了")
        .collect::<Result<_, _>>()
        .expect("读得出")
}

#[tokio::test]
async fn a_turn_said_through_the_core_is_indexed_and_goes_with_the_session() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([Play::Says("好，记住你用 N 卡。")])));
    client.hello().await;
    let work = home.work.to_string_lossy().into_owned();
    let session = client.create_as("c1", &work, "engineer").await;
    client.say("c2", &session, "我的显卡是 N 卡").await;
    home.until_turns(&session, 1).await;
    // 回合库在日志落了盘以后才更新：等它出现，最多六十秒（机器忙时慢，只在出错时等满）。
    let mut keys = found(&home, "显卡");
    for _ in 0..1200 {
        if !keys.is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        keys = found(&home, "显卡");
    }
    assert_eq!(keys.len(), 1, "{keys:?}");
    assert!(keys[0].starts_with(&format!("{session}/")), "{keys:?}");

    let reply = client
        .call("c3", "session.delete", json!({"session": session}))
        .await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    assert!(found(&home, "显卡").is_empty(), "删会话拿掉");
    let path = home
        .root
        .index(&alice())
        .join("recall")
        .join("turns-engineer.db");
    let db =
        rusqlite::Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .expect("回合库在");
    let buried: i64 = db
        .query_row(
            "SELECT count(*) FROM buried WHERE key = ?1",
            [format!("{session}/")],
            |row| row.get(0),
        )
        .expect("查得了");
    assert_eq!(buried, 1, "整个会话埋了墓碑：记忆的出处在它里面的都算死了");
}

/// 常驻的摘要接在核心上（施工 R-4 上，`memory.md` 第三条第 5 款）：核心起来时读好外壳的字交给记忆；人经协议记了一条，新会话
/// 第一轮的请求里有那一块。
#[tokio::test]
async fn a_new_session_starts_with_what_was_remembered() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。")]);
    let mut client = crate::support::memories::connected(&home, &script).await;
    let reply = client
        .call(
            "c1",
            "memory.remember",
            json!({"class": "user", "text": "用户养了一只猫"}),
        )
        .await;
    assert_eq!(reply["result"], json!({"id": "m1"}), "{reply}");
    let work = home.work.to_string_lossy().into_owned();
    let session = client.create_as("c2", &work, "engineer").await;
    client.say("c3", &session, "在吗").await;
    home.until_turns(&session, 1).await;
    let request = format!("{:?}", script.requests()[0].1);
    assert!(
        request.contains(r"<memories>\nm1 user ")
            && request.contains(r": 用户养了一只猫\n</memories>"),
        "{request}"
    );
}
