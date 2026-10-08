//! 会话认自己的属主（施工 O-4 上，`docs/construction/O-4-系统账号（上）.md`）：没在跑的会话照数据根里哪个账号的
//! `home/<账号>/sessions/<编号>/` 在找，载入、读页、接着说话、删掉都在那个账号的家目录下，管理员的家目录一个字不动。
//! 造会话还只能是管理员（连进来的人），所以把造好的会话的目录挪到另一个账号下，换一个核心再找。

use std::path::PathBuf;

use serde_json::json;

use miyu_kernel::event::Body;
use miyu_kernel::id::{AccountId, SessionId};
use miyu_session::testkit::{Play, Script};
use miyu_store::log::read_events;

use crate::support::*;

/// 会话 `session` 在账号 `account` 名下的目录。
fn dir(home: &Home, account: &str, session: &str) -> PathBuf {
    home.root.session_dir(
        &AccountId::parse(account).expect("账号合写法"),
        &SessionId::parse(session).expect("会话编号合写法"),
    )
}

/// `dir` 里的日志结束了几轮。
fn turns_in(dir: &std::path::Path) -> usize {
    read_events(dir)
        .unwrap_or_default()
        .iter()
        .filter(|event| matches!(event.body, Body::TurnEnded(_)))
        .count()
}

#[tokio::test]
async fn a_session_under_another_account_is_found_loaded_and_deleted_there() {
    let home = Home::new();
    let script = Script::new([Play::Says("一。"), Play::Says("二。")]);
    let first = home.core(&script);
    let mut client = Client::connect(first.clone());
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("s1", &session, "a").await;
    home.until_turns(&session, 1).await;
    drop(client);
    first.stop_sessions().await;

    let (alices, bobs) = (dir(&home, "alice", &session), dir(&home, "bob", &session));
    std::fs::create_dir_all(bobs.parent().expect("有上一级")).expect("建得了");
    std::fs::rename(&alices, &bobs).expect("挪得动");
    // 策略快照这些 blob 也照属主放：系统账号的会话本来就在它自己的家目录下造，这里一起拷过去。
    let (alice, bob) = (
        AccountId::parse("alice").expect("合写法"),
        AccountId::parse("bob").expect("合写法"),
    );
    copy_tree(&home.root.blobs(&alice), &home.root.blobs(&bob));

    let second = home.core(&script);
    let mut client = Client::connect(second.clone());
    client.hello().await;
    let page = client
        .call("p1", "view.page", json!({"session": session, "turns": 1}))
        .await;
    assert!(
        page["result"]["events"]
            .as_array()
            .is_some_and(|events| !events.is_empty()),
        "照属主找得到它的日志：{page}"
    );
    // 订阅另开一个连接：推送不跟后面的回应搅在一起。
    let mut watcher = Client::connect(second.clone());
    watcher.hello().await;
    let subscribed = watcher.subscribe("x1", &session).await;
    assert!(
        subscribed["result"]["preset"].is_string(),
        "订阅的回应照属主的日志读 session.created：{subscribed}"
    );
    drop(watcher);
    let said = client.say("s2", &session, "b").await;
    assert!(said.get("error").is_none(), "照属主载入：{said}");
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(60);
    while turns_in(&bobs) < 2 {
        assert!(
            tokio::time::Instant::now() < deadline,
            "第二轮写在 bob 的家目录下"
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert!(!alices.exists(), "管理员的家目录一个字没动");

    let deleted = client
        .call("d1", "session.delete", json!({"session": session}))
        .await;
    assert_eq!(deleted["result"], json!({}), "{deleted}");
    let trash = |account: &str| {
        home.root
            .path()
            .join("home")
            .join(account)
            .join("trash/sessions")
            .join(&session)
    };
    assert!(trash("bob").is_dir(), "进 bob 的回收处");
    assert!(!trash("alice").exists());
    assert!(!bobs.exists());
}

/// 把 `from` 整棵拷到 `to`。
fn copy_tree(from: &std::path::Path, to: &std::path::Path) {
    std::fs::create_dir_all(to).expect("建得了");
    for entry in std::fs::read_dir(from).expect("读得了") {
        let entry = entry.expect("读得了");
        let target = to.join(entry.file_name());
        match entry.file_type().expect("有类型").is_dir() {
            true => copy_tree(&entry.path(), &target),
            false => {
                std::fs::copy(entry.path(), &target).expect("拷得了");
            }
        }
    }
}
