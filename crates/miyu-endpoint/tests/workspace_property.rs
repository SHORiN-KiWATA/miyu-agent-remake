//! 工作区是会话的属性（施工 9-7 上，`docs/blueprint/protocol.md`「`session.set_workspace`」）：一个头换了，订阅着的另一个头
//! 收到 `session.workspace_changed`、会话列表那一项跟着变；订阅的回应带 `workspace`；一样的不记；写错的当场拒绝。

mod support;

use serde_json::{Value, json};

use miyu_kernel::event::Body;
use miyu_session::testkit::Script;
use support::*;

/// 工作区旁边建一个目录，交回它的路径（数据根外面）。
fn dir(home: &Home, name: &str) -> String {
    let dir = home.work.join(name);
    std::fs::create_dir_all(&dir).expect("建得了目录");
    dir.to_string_lossy().into_owned()
}

/// 读到一条推送、合 `wanted` 为止，交回它：回应跳过，最多 60 秒。
async fn until_pushed(client: &mut Client, wanted: impl Fn(&Value) -> bool) -> Value {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(60);
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        let message = client
            .next_within(left)
            .await
            .unwrap_or_else(|| panic!("等不到推送"));
        if wanted(&message) {
            return message;
        }
    }
}

#[tokio::test]
async fn one_head_moves_it_and_the_others_follow() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    let start = dir(&home, "start");
    let other = dir(&home, "other");
    let mut first = Client::connect(core.clone());
    first.hello().await;
    let session = first.create("c1", &start).await;
    let mut second = Client::connect(core);
    second.hello().await;
    let watched = second.subscribe("w1", &session).await;
    assert_eq!(
        watched["result"]["workspace"],
        json!({"cwd": start, "dirs": []}),
        "{watched}"
    );
    second
        .call("w2", "subscribe", json!({"stream": "sessions"}))
        .await;

    let moved = first
        .call(
            "m1",
            "session.set_workspace",
            json!({"session": session, "cwd": other, "dirs": [start]}),
        )
        .await;
    assert_eq!(
        moved["result"],
        json!({"cwd": other, "dirs": [start]}),
        "{moved}"
    );
    let pushed = until_pushed(&mut second, |message| {
        message["params"]["event"]["kind"] == "session.workspace_changed"
    })
    .await;
    assert_eq!(
        pushed["params"]["event"]["body"],
        json!({"cwd": other, "dirs": [start]})
    );
    let listed = until_pushed(&mut second, |message| {
        message["method"] == "sessions.changed" && message["params"]["entry"]["cwd"] == json!(other)
    })
    .await;
    assert_eq!(listed["params"]["session"], json!(session));
    let again = second.subscribe("w3", &session).await;
    assert_eq!(
        again["result"]["workspace"],
        json!({"cwd": other, "dirs": [start]})
    );

    // 一样的：接受，什么都不记。
    let same = first
        .call(
            "m2",
            "session.set_workspace",
            json!({"session": session, "cwd": other}),
        )
        .await;
    assert_eq!(same["result"]["cwd"], json!(other), "{same}");
    let changes = home
        .log(&session)
        .iter()
        .filter(|event| matches!(event.body, Body::WorkspaceChanged(_)))
        .count();
    assert_eq!(changes, 1);
}

#[tokio::test]
async fn wrong_places_are_refused_on_the_spot() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let start = dir(&home, "start");
    let session = client.create("c1", &start).await;
    let file = home.work.join("note.txt");
    std::fs::write(&file, "x").expect("写得进");
    let state = home.root.state().to_string_lossy().into_owned();
    for (n, (cwd, reason_wanted)) in [
        (
            home.work.join("nope").to_string_lossy().into_owned(),
            "path_unreadable",
        ),
        (file.to_string_lossy().into_owned(), "not_a_directory"),
        (state, "path_forbidden"),
    ]
    .into_iter()
    .enumerate()
    {
        let reply = client
            .call(
                &format!("r{n}"),
                "session.set_workspace",
                json!({"session": session, "cwd": cwd}),
            )
            .await;
        assert_eq!(reason(&reply), Some(reason_wanted), "{cwd}：{reply}");
    }
    for (n, params) in [
        json!({"session": session}),
        json!({"session": session, "cwd": start, "more": 1}),
        json!({"session": "nope", "cwd": start}),
    ]
    .into_iter()
    .enumerate()
    {
        let reply = client
            .call(&format!("b{n}"), "session.set_workspace", params.clone())
            .await;
        assert_eq!(reason(&reply), Some("bad_params"), "{params}：{reply}");
    }
    let missing = client
        .call(
            "m",
            "session.set_workspace",
            json!({"session": "01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91", "cwd": start}),
        )
        .await;
    assert_eq!(reason(&missing), Some("session_not_found"), "{missing}");
    assert!(
        !home
            .log(&session)
            .iter()
            .any(|event| matches!(event.body, Body::WorkspaceChanged(_))),
        "拒绝的什么都没记"
    );
}

#[tokio::test]
async fn a_change_survives_a_restart_before_any_turn() {
    let home = Home::new();
    let script = Script::new([]);
    let start = dir(&home, "start");
    let other = dir(&home, "other");
    let extra = dir(&home, "extra");
    let first = home.core(&script);
    let mut client = Client::connect(first.clone());
    client.hello().await;
    let session = client.create("c1", &start).await;
    client
        .call(
            "m1",
            "session.set_workspace",
            json!({"session": session, "cwd": other, "dirs": [extra]}),
        )
        .await;
    first.stop_sessions().await;
    drop(client);
    // 核心重启：还没开过轮，工作区只记在 `session.workspace_changed` 里。
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let watched = client.subscribe("w1", &session).await;
    assert_eq!(
        watched["result"]["workspace"],
        json!({"cwd": other, "dirs": [extra]}),
        "{watched}"
    );
}
