//! 加进来的目录（施工 5-10 上，`docs/blueprint/protocol.md`「加进来的目录」）：造会话、说话时报来，记进这一轮的
//! `turn.started`；`dirs` 不写的照旧，写空的就是没有；太宽的整条命令都不收；核心重启以后照最后一轮的找回来。

mod support;

use serde_json::{Value, json};

use miyu_kernel::event::{Body, Event};
use miyu_session::testkit::{Play, Script};
use support::*;

/// 工作区旁边建一个目录，交回它的路径（数据根外面）。
fn extra(home: &Home, name: &str) -> String {
    let dir = home.work.join(name);
    std::fs::create_dir_all(&dir).expect("建得了目录");
    dir.to_string_lossy().into_owned()
}

/// 每一轮 `turn.started` 带的加进来的目录，照先后。
fn dirs_of_turns(log: &[Event]) -> Vec<Vec<String>> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::TurnStarted(started) => Some(started.dirs.clone()),
            _ => None,
        })
        .collect()
}

/// 造会话的回应里的会话编号。
fn session_of(reply: &Value) -> String {
    reply["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("应该造出会话：{reply}"))
        .to_string()
}

#[tokio::test]
async fn added_dirs_go_into_the_turn_and_stay_until_told_otherwise() {
    let home = Home::new();
    let extra = extra(&home, "extra");
    let script = Script::new([Play::Says("好。"), Play::Says("好。"), Play::Says("好。")]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let cwd = home.work.to_string_lossy().into_owned();
    let created = client
        .call(
            "create-1",
            "session.create",
            json!({"cwd": cwd, "dirs": [extra]}),
        )
        .await;
    let session = session_of(&created);
    client.say("send-1", &session, "hi").await;
    home.until_turns(&session, 1).await;
    client.say("send-2", &session, "again").await;
    home.until_turns(&session, 2).await;
    let cleared = json!({"session": session, "text": "clear", "dirs": []});
    client.call("send-3", "session.send", cleared).await;
    home.until_turns(&session, 3).await;
    assert_eq!(
        dirs_of_turns(&home.log(&session)),
        [vec![extra.clone()], vec![extra], Vec::new()],
        "造会话时带的、不写的照旧、写空的就没有了"
    );
}

#[tokio::test]
async fn too_wide_dirs_are_refused_before_anything_is_written() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。")]);
    // 假的家就是工作区的上一级：它太宽。
    let fake_home = home.work.parent().expect("有上一级").to_path_buf();
    let mut client = Client::connect(home.core_at_home(&script, fake_home.clone()));
    client.hello().await;
    let cwd = home.work.to_string_lossy().into_owned();
    let root = if cfg!(windows) { "C:\\" } else { "/" };
    let wide = [
        "~".to_string(),
        fake_home.to_string_lossy().into_owned(),
        root.to_string(),
        home.root.path().to_string_lossy().into_owned(),
        home.root
            .path()
            .join("state")
            .to_string_lossy()
            .into_owned(),
    ];
    for (n, dir) in wide.iter().enumerate() {
        let reply = client
            .call(
                &format!("create-{n}"),
                "session.create",
                json!({"cwd": cwd, "dirs": [extra(&home, "fine"), dir]}),
            )
            .await;
        assert_eq!(reason(&reply), Some("dir_too_wide"), "{dir}：{reply}");
    }
    let session = client.create("create-ok", &cwd).await;
    let send = json!({"session": session, "text": "hi", "dirs": [wide[3]]});
    let reply = client.call("send-wide", "session.send", send).await;
    assert_eq!(reason(&reply), Some("dir_too_wide"), "{reply}");
    assert!(
        !home
            .log(&session)
            .iter()
            .any(|event| matches!(event.body, Body::MessageUser(_))),
        "这一句没收下"
    );
}

#[tokio::test]
async fn after_a_restart_the_last_turns_dirs_are_kept() {
    let home = Home::new();
    let extra = extra(&home, "extra");
    let script = Script::new([Play::Says("好。"), Play::Says("好。")]);
    let first = home.core(&script);
    let mut client = Client::connect(first.clone());
    client.hello().await;
    let cwd = home.work.to_string_lossy().into_owned();
    let session = client.create("create-r", &cwd).await;
    let send = json!({"session": session, "text": "hi", "dirs": [extra]});
    client.call("send-r1", "session.send", send).await;
    home.until_turns(&session, 1).await;
    first.stop_sessions().await;
    drop(client);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    client.say("send-r2", &session, "again").await;
    home.until_turns(&session, 2).await;
    assert_eq!(
        dirs_of_turns(&home.log(&session)),
        [vec![extra.clone()], vec![extra]],
        "重启以后第一次说话不带 dirs：照最后一轮的"
    );
}
