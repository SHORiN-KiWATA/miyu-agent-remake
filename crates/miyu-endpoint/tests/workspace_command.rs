//! 斜杠命令 `/workspace`（施工 9-7 下，`docs/blueprint/protocol.md` 的 `command.run`）：真核心走一遍。`/workspace <路径>` 同
//! `session.set_workspace` 只换工作目录，加进来的目录照旧；相对的照 `command.run` 带的 `cwd`（头所在的目录）接，没带的照会话
//! 现在的工作区接；不带路径的只说现在在哪。写错的照那几种原因拒绝、什么都不记；太宽的退回账号的工作区，回执说一声。场所里
//! 只有主人能换。

mod support;

use std::path::Path;

use serde_json::{Value, json};

use miyu_kernel::event::{Body, Event};
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use support::venues::bound_core;
use support::*;

async fn run(client: &mut Client, id: &str, params: Value) -> Value {
    client.call(id, "command.run", params).await
}

/// 工作区旁边建一个目录，交回它的路径。
fn dir(home: &Home, name: &str) -> String {
    let dir = home.work.join(name);
    std::fs::create_dir_all(&dir).expect("建得了目录");
    dir.to_string_lossy().into_owned()
}

/// 真实的位置，人看到的写法。
fn real(path: &str) -> String {
    std::fs::canonicalize(Path::new(path))
        .expect("在")
        .to_string_lossy()
        .into_owned()
}

/// 日志里每一次换到的工作目录。
fn moves(log: &[Event]) -> Vec<String> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::WorkspaceChanged(changed) => Some(changed.cwd.clone()),
            _ => None,
        })
        .collect()
}

/// 日志里记下的命令的原文。
fn noted(log: &[Event]) -> Vec<String> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::CommandRan(ran) => Some(ran.text.clone()),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn it_moves_the_workspace_like_the_method_and_keeps_the_dirs() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let start = dir(&home, "start");
    let other = dir(&home, "other");
    let extra = dir(&home, "extra");
    let session = client.create("c1", &start).await;
    client
        .call(
            "d1",
            "session.set_workspace",
            json!({"session": session, "dirs": [extra]}),
        )
        .await;

    let text = format!("/workspace {other}");
    let reply = run(&mut client, "k1", json!({"session": session, "text": text})).await;
    assert_eq!(reply["result"]["command"], "workspace", "{reply}");
    assert_eq!(
        reply["result"]["said"],
        json!(format!("工作区换到了 {other}。"))
    );
    let log = home.log(&session);
    assert_eq!(moves(&log), [start.clone(), other.clone()]);
    assert_eq!(noted(&log), [text]);
    let events: Vec<u64> = serde_json::from_value(reply["result"]["events"].clone()).expect("序号");
    assert_eq!(events.len(), 2, "换了的一条、记下的一条：{reply}");

    // 一样的：照样记下命令，不再记换。
    let again = run(
        &mut client,
        "k2",
        json!({"session": session, "text": format!("/workspace   {other}  ")}),
    )
    .await;
    assert_eq!(
        again["result"]["said"],
        json!(format!("工作区换到了 {other}。")),
        "{again}"
    );
    assert_eq!(moves(&home.log(&session)).len(), 2);
    let now = run(
        &mut client,
        "k3",
        json!({"session": session, "text": "/workspace"}),
    )
    .await;
    assert_eq!(
        now["result"]["said"],
        json!(format!("现在的工作区是 {other}。")),
        "会话表跟着换了：{now}"
    );
    let watched = client.subscribe("w1", &session).await;
    assert_eq!(
        watched["result"]["workspace"],
        json!({"cwd": other, "dirs": [extra]}),
        "加进来的目录照旧"
    );
}

#[tokio::test]
async fn relative_paths_follow_the_head_and_then_the_session() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let start = dir(&home, "start");
    let near = dir(&home, "start/near");
    let head = dir(&home, "head");
    let spaced = dir(&home, "head/with space");
    let session = client.create("c1", &start).await;

    let reply = run(
        &mut client,
        "k1",
        json!({"session": session, "text": "/workspace near"}),
    )
    .await;
    assert_eq!(reply["result"]["command"], "workspace", "{reply}");
    let reply = run(
        &mut client,
        "k2",
        json!({"session": session, "text": "/workspace .", "cwd": head}),
    )
    .await;
    assert_eq!(reply["result"]["command"], "workspace", "{reply}");
    let reply = run(
        &mut client,
        "k3",
        json!({"session": session, "text": "/workspace with space", "cwd": head}),
    )
    .await;
    assert_eq!(reply["result"]["command"], "workspace", "{reply}");
    assert_eq!(
        moves(&home.log(&session)),
        [real(&near), real(&head), real(&spaced)],
        "没带 cwd 的照会话的接；带了的照头的接；路径里的空白照留"
    );

    let reply = run(
        &mut client,
        "k4",
        json!({"session": session, "text": "/workspace .", "cwd": "relative/dir"}),
    )
    .await;
    assert_eq!(
        reason(&reply),
        Some("bad_params"),
        "头的目录要是绝对的：{reply}"
    );
}

#[tokio::test]
async fn without_a_path_it_only_says_where() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let start = dir(&home, "start");
    let session = client.create("c1", &start).await;
    let reply = run(
        &mut client,
        "k1",
        json!({"session": session, "text": "/workspace  "}),
    )
    .await;
    assert_eq!(
        reply["result"]["said"],
        json!(format!("现在的工作区是 {start}。")),
        "{reply}"
    );
    let log = home.log(&session);
    assert!(moves(&log).is_empty(), "什么都不换");
    assert_eq!(noted(&log), ["/workspace  "]);
}

#[tokio::test]
async fn wrong_places_are_refused_and_too_wide_falls_back() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let start = dir(&home, "start");
    let session = client.create("c1", &start).await;
    let file = home.work.join("note.txt");
    std::fs::write(&file, "x").expect("写得进");
    let state = home.root.state().to_string_lossy().into_owned();
    for (n, (path, wanted)) in [
        ("nope".to_string(), "path_unreadable"),
        (file.to_string_lossy().into_owned(), "not_a_directory"),
        (state, "path_forbidden"),
    ]
    .into_iter()
    .enumerate()
    {
        let reply = run(
            &mut client,
            &format!("r{n}"),
            json!({"session": session, "text": format!("/workspace {path}")}),
        )
        .await;
        assert_eq!(reason(&reply), Some(wanted), "{path}：{reply}");
    }
    let log = home.log(&session);
    assert!(noted(&log).is_empty(), "拒绝的什么都不记");
    assert!(moves(&log).is_empty());

    let reply = run(
        &mut client,
        "w",
        json!({"session": session, "text": "/workspace ~"}),
    )
    .await;
    let own = home.root.workspace(&alice()).to_string_lossy().into_owned();
    assert_eq!(
        reply["result"]["said"],
        json!(format!("~ 太宽，工作区换到了 {own}。")),
        "{reply}"
    );
    assert_eq!(moves(&home.log(&session)).last(), Some(&own));
}

#[tokio::test]
async fn in_a_venue_only_the_owner_may_move_it() {
    let home = Home::new();
    let script = Script::new([Play::Says("在。")]);
    let mut client = Client::connect(bound_core(&home, &script, Catalog::default()));
    client.hello().await;
    let made = client
        .call(
            "v1",
            "venue.session",
            json!({"venue": "qq:private:10001", "kind": "private", "peer": "qq:10001"}),
        )
        .await;
    let session = made["result"]["session"]
        .as_str()
        .expect("有编号")
        .to_string();
    let other = dir(&home, "other");
    let text = format!("/workspace {other}");
    let manager = json!({"external": "qq:10003", "role": "manager"});
    let reply = run(
        &mut client,
        "k1",
        json!({"session": session, "text": text, "as": manager}),
    )
    .await;
    assert_eq!(reason(&reply), Some("owner_only"), "{reply}");
    assert_eq!(reply["error"]["message"], "只有主人能用这个命令。");
    let owner = json!({"external": "qq:10001"});
    let reply = run(
        &mut client,
        "k2",
        json!({"session": session, "text": text, "as": owner}),
    )
    .await;
    assert_eq!(reply["result"]["command"], "workspace", "{reply}");
    assert_eq!(moves(&home.log(&session)).last(), Some(&other));
}
