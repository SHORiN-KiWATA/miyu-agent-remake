//! 回合索引接在核心上（施工 R-2 上，`docs/blueprint/memory.md`「怎么走」第一条第 4、7 款）：真核心造的主会话说过的一轮进
//! 软件工程师的回合库；删会话以后拿掉。

mod support;

use serde_json::json;

use miyu_session::testkit::{Play, Script};
use miyu_store::recall::RecallIndex;

use support::*;

/// 另开一个连接，在软件工程师的回合库里搜 `words`，交回键。
fn found(home: &Home, words: &str) -> Vec<String> {
    let path = home
        .root
        .index(&alice())
        .join("recall")
        .join("turns-engineer.db");
    let (index, _) = RecallIndex::open(&path);
    index
        .search(words, 10)
        .expect("搜得了")
        .into_iter()
        .map(|hit| hit.key)
        .collect()
}

#[tokio::test]
async fn a_turn_said_through_the_core_is_indexed_and_goes_with_the_session() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([Play::Says("好，记住你用 N 卡。")])));
    client.hello().await;
    let work = home.work.to_string_lossy().into_owned();
    let session = client.create("c1", &work).await;
    client.say("c2", &session, "我的显卡是 N 卡").await;
    home.until_turns(&session, 1).await;
    // 回合库在日志落了盘以后才更新：等它出现，最多五秒。
    let mut keys = found(&home, "显卡");
    for _ in 0..100 {
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
}
