//! 会话列表的索引（施工 3-8 七补，`docs/blueprint/store/index.md`）：造会话、说话、改标题、置顶以后，索引里那一行和日志对得上，
//! 照到的就是日志的末尾；删会话删掉那一行。

mod support;

use serde_json::json;

use miyu_kernel::id::SessionId;
use miyu_session::testkit::{Play, Script};
use miyu_store::index::{FILE, Row, SessionIndex};

use support::*;

/// 索引里会话 `session` 那一行：另开一个连接读。
fn row(home: &Home, session: &str) -> Option<Row> {
    let (index, _) = SessionIndex::open(&home.root.index(&alice()).join(FILE));
    let id = SessionId::parse(session).expect("合写法");
    index.rows().expect("读得了").remove(&id)
}

#[tokio::test]
async fn the_row_follows_what_the_session_writes_and_goes_with_it() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([Play::Says("好。")])));
    client.hello().await;
    let work = home.work.to_string_lossy().into_owned();
    let session = client.create("c1", &work).await;
    let made = row(&home, &session).expect("造好就有这一行");
    assert_eq!((made.title.as_str(), made.pinned), ("", false));
    assert_eq!(made.cwd.as_deref(), Some(work.as_str()));
    client.say("c2", &session, "hi").await;
    home.until_turns(&session, 1).await;
    let reply = client
        .call(
            "c3",
            "session.set_meta",
            json!({"session": session, "title": "发版", "pinned": true}),
        )
        .await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    // 回应在落了盘、更新了索引以后：这之前的每一批都盖上了。
    let log = home.log(&session);
    let got = row(&home, &session).expect("有这一行");
    assert_eq!((got.title.as_str(), got.pinned), ("发版", true));
    assert_eq!(got.cwd.as_deref(), Some(work.as_str()));
    assert_eq!((got.oneshot, got.parent.as_ref()), (false, None));
    assert_eq!(got.owner, alice());
    assert_eq!(got.created, log[0].at);
    assert_eq!(got.last_active, log.last().expect("有事件").at);
    let segment = home
        .root
        .session_dir(&alice(), &got.id)
        .join("000000000001.jsonl");
    let bytes = std::fs::metadata(segment).expect("在").len();
    assert_eq!(
        (got.mark.segment, got.mark.bytes),
        (1, bytes),
        "照到日志的末尾"
    );
    assert_eq!(got.mark.next.get(), log.len() as u64 + 1);
    let reply = client.call("c4", "session.list", json!({})).await;
    let item = &reply["result"]["sessions"][0];
    assert_eq!(
        (&item["title"], &item["pinned"]),
        (&json!("发版"), &json!(true))
    );
    let reply = client
        .call("c5", "session.delete", json!({"session": session}))
        .await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    assert!(row(&home, &session).is_none(), "删会话删那一行");
}
