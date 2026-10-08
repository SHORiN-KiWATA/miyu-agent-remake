//! 核心重启以后（施工 4-9 再补三上）：不带 `cwd` 载入的会话，工作目录照日志里最后记下的；重发的造会话认得出来。
//! 同一个数据根上换一份核心，就像重启过。

mod support;

use std::path::Path;

use serde_json::json;

use miyu_session::testkit::{Play, Script};
use support::*;

/// 给人看的路径：Windows 上去掉 `\\?\` 这个前缀，和回应里写的一样。
fn plain(path: &Path) -> String {
    let text = path.to_string_lossy();
    text.strip_prefix(r"\\?\").unwrap_or(&text).to_string()
}

/// 重启以后第一次用到会话的是撤销（不带 `cwd`）：工作目录照最后一轮的 `turn.started`，回应里的 `cwd` 是它，
/// `miyu undo` 才写得短路径。造会话时报的是 `~`（退回管理员的工作区），这一轮报的是项目目录：照的是这一轮的。
#[tokio::test]
async fn a_session_loaded_without_a_cwd_keeps_the_last_one() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。")]);
    let first = home.core(&script);
    let mut client = Client::connect(first.clone());
    client.hello().await;
    let session = client.create("create-a", "~").await;
    // 换了工作区（施工 9-7 上：说话不再换它）：重启以后照记下的那条找回来。
    let cwd = home.work.to_string_lossy().into_owned();
    let moved = json!({"session": session, "cwd": cwd});
    client.call("move-a", "session.set_workspace", moved).await;
    client.say("send-a", &session, "hi").await;
    home.until_turns(&session, 1).await;
    first.stop_sessions().await;
    drop(client);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let reply = client
        .call("undo-a", "session.revert", json!({"session": session}))
        .await;
    assert_eq!(reply["result"]["cwd"], json!(plain(&home.work)), "{reply}");
}

/// 还没开过回合的会话：照 `session.created` 的工作目录载入。
#[tokio::test]
async fn a_session_without_turns_keeps_the_cwd_it_was_made_in() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。")]);
    let first = home.core(&script);
    let mut client = Client::connect(first.clone());
    client.hello().await;
    let cwd = home.work.to_string_lossy().into_owned();
    let session = client.create("create-b", &cwd).await;
    first.stop_sessions().await;
    drop(client);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    client.subscribe("watch-b", &session).await;
    let send = json!({"jsonrpc": "2.0", "id": "send-b", "method": "session.send", "params": {"session": session, "text": "hi"}});
    client.line(&send.to_string()).await;
    let (_, reply) = client.until_reply("send-b").await;
    // 说话的回应里写的是实际干活的目录，照报来的原样（撤销的回应才去掉 Windows 的 `\\?\` 前缀）。
    assert_eq!(reply["result"]["cwd"], json!(cwd), "{reply}");
}

/// 重启以后重发同一个造会话：交回原来那一个，不再造；别的编号照样造新的。
#[tokio::test]
async fn the_same_create_after_a_restart_is_the_same_session() {
    let home = Home::new();
    let script = Script::new([]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let first = client.create("create-once", "~").await;
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    assert_eq!(client.create("create-once", "~").await, first);
    assert_eq!(
        home.root.sessions(&alice()).expect("列得出").len(),
        1,
        "只造了一个"
    );
    assert_ne!(client.create("create-other", "~").await, first);
}
