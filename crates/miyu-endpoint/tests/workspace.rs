//! 工作目录太宽（施工 4-3 下，`11-权限与沙盒.md` 第四节）：头报来的是 `~`、系统的家目录、根目录，或者包含
//! 数据根、落在数据根里的，退回管理员的工作区 `home/<账号>/workspace/`；项目目录照旧。造会话、说话的回应
//! 说会话实际在哪个目录里干活（施工 4-5 下）。施工 9-7 上起说话不再换工作区，换的是 `session.set_workspace`。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Value, json};

use crate::support::{Client, Home, alice};
use miyu_kernel::template::escape;
use miyu_session::testkit::{Play, Script};

/// 一个用完就删的临时目录：假的家、项目目录放在数据根外面。
struct Outside(PathBuf);

impl Outside {
    fn new() -> Outside {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("miyu-endpoint-out-{}-{n}", std::process::id()));
        for sub in ["home", "proj"] {
            std::fs::create_dir_all(dir.join(sub)).expect("建得了目录");
        }
        Outside(dir)
    }
}

impl Drop for Outside {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "删不掉就留在临时目录里，不影响测试"
    )]
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// 请求里全部的字，接成一段。
fn words(value: &Value, out: &mut String) {
    match value {
        Value::String(text) => out.push_str(text),
        Value::Array(items) => items.iter().for_each(|item| words(item, out)),
        Value::Object(fields) => fields.values().for_each(|field| words(field, out)),
        _ => {}
    }
}

/// 头报来的工作目录是 `cwd`：她看到的请求里的字。每次起一个新的核心，数据根是同一个：命令编号要全局不撞
/// （`04-核心协议.md` 第六节第 1 条），核心重启以后照样认得出重发的造会话，所以每次换一对编号。
async fn seen(home: &Home, outside: &Outside, cwd: &str) -> String {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let script = Script::new([Play::Says("好。")]);
    let mut client = Client::connect(home.core_at_home(&script, outside.0.join("home")));
    client.hello().await;
    let session = client.create(&format!("create-{n}"), cwd).await;
    client
        .call(
            &format!("send-{n}"),
            "session.send",
            json!({"session": session, "text": "hi"}),
        )
        .await;
    home.until_turns(&session, 1).await;
    let request = script.requests()[0].1.clone();
    let value: Value = serde_json::from_slice(&request.canonical_bytes()).expect("请求是 JSON");
    let mut text = String::new();
    words(&value, &mut text);
    text
}

/// 环境事实里写的工作目录：照事实模板转义过。
fn written(path: &Path) -> String {
    escape(&path.to_string_lossy())
}

#[tokio::test]
async fn a_directory_too_wide_falls_back_to_the_account_workspace() {
    let home = Home::new();
    let outside = Outside::new();
    let own = home.root.workspace(&alice());
    let root = if cfg!(windows) { "C:\\" } else { "/" };
    let inside_data = home.root.path().join("state");
    for cwd in [
        "~".to_string(),
        outside.0.join("home").to_string_lossy().into_owned(),
        root.to_string(),
        home.root.path().to_string_lossy().into_owned(),
        inside_data.to_string_lossy().into_owned(),
    ] {
        let text = seen(&home, &outside, &cwd).await;
        assert!(text.contains(&written(&own)), "{cwd}：{text}");
    }
    assert!(own.is_dir(), "退回的工作区建好了");
    // 核心读不出家目录：`~` 本身照样太宽。
    let script = Script::new([Play::Says("好。")]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c9", "~").await;
    client
        .call(
            "c10",
            "session.send",
            json!({"session": session, "text": "hi"}),
        )
        .await;
    home.until_turns(&session, 1).await;
    let value: Value =
        serde_json::from_slice(&script.requests()[0].1.canonical_bytes()).expect("请求是 JSON");
    let mut text = String::new();
    words(&value, &mut text);
    assert!(text.contains(&written(&own)), "{text}");
}

#[tokio::test]
async fn a_project_directory_or_the_account_workspace_stays() {
    let home = Home::new();
    let outside = Outside::new();
    let project = outside.0.join("proj");
    let text = seen(&home, &outside, &project.to_string_lossy()).await;
    assert!(text.contains(&written(&project)), "{text}");
    let own = home.root.workspace(&alice());
    assert!(!text.contains(&written(&own)), "{text}");
    // 账号自己的工作区在数据根里，照样当工作区。
    std::fs::create_dir_all(&own).expect("建得了");
    let text = seen(&home, &outside, &own.to_string_lossy()).await;
    assert!(text.contains(&written(&own)), "{text}");
}

#[tokio::test]
async fn the_replies_say_where_the_session_works() {
    let home = Home::new();
    let outside = Outside::new();
    let own = home.root.workspace(&alice()).to_string_lossy().into_owned();
    let project = outside.0.join("proj").to_string_lossy().into_owned();
    let script = Script::new([Play::Says("好。"), Play::Says("好。")]);
    let mut client = Client::connect(home.core_at_home(&script, outside.0.join("home")));
    client.hello().await;
    let reply = client
        .call("c1", "session.create", json!({"cwd": project}))
        .await;
    assert_eq!(
        reply["result"]["cwd"],
        json!(project),
        "项目目录照原样：{reply}"
    );
    let session = reply["result"]["session"]
        .as_str()
        .expect("造出来了")
        .to_string();
    // 说话带着目录的不再换工作区（施工 9-7 上）：照收不理，回应里是会话现在的。
    let reply = client
        .call(
            "c2",
            "session.send",
            json!({"session": session, "text": "hi", "cwd": "~"}),
        )
        .await;
    assert_eq!(reply["result"]["cwd"], json!(project), "{reply}");
    home.until_turns(&session, 1).await;
    // 人明着换到太宽的目录：照人选的用，回应带 `wide`（施工 9-7 补，原来退回账号的工作区）。
    let reply = client
        .call(
            "w1",
            "session.set_workspace",
            json!({"session": session, "cwd": "~"}),
        )
        .await;
    assert_eq!(reply["result"]["cwd"], json!("~"), "{reply}");
    assert_eq!(reply["result"]["wide"], json!(true), "{reply}");
    // 之后说话的回应，是会话现在的。
    let reply = client
        .call(
            "c3",
            "session.send",
            json!({"session": session, "text": "hi"}),
        )
        .await;
    assert_eq!(reply["result"]["cwd"], json!("~"), "{reply}");
    home.until_turns(&session, 2).await;
    // 造会话时就太宽的、头自己带上的：照旧退回；同一个命令编号重发的，说的一样。
    for _ in 0..2 {
        let reply = client
            .call("c4", "session.create", json!({"cwd": "~"}))
            .await;
        assert_eq!(reply["result"]["cwd"], json!(own), "{reply}");
        assert!(reply["result"].get("wide").is_none(), "{reply}");
    }
    // 人明着选的：照用，带 `wide`。
    let reply = client
        .call("c5", "session.create", json!({"cwd": "~", "chosen": true}))
        .await;
    assert_eq!(reply["result"]["cwd"], json!("~"), "{reply}");
    assert_eq!(reply["result"]["wide"], json!(true), "{reply}");
    // 明着选了数据根里面、又不是账号自己的工作区的：照不了，照旧退回。
    let state = home.root.state().to_string_lossy().into_owned();
    let reply = client
        .call(
            "c7",
            "session.create",
            json!({"cwd": state, "chosen": true}),
        )
        .await;
    assert_eq!(reply["result"]["cwd"], json!(own), "{reply}");
    assert!(reply["result"].get("wide").is_none(), "{reply}");
    // 不太宽的明着选，和不选一样，不带 `wide`。
    let reply = client
        .call(
            "c6",
            "session.create",
            json!({"cwd": project, "chosen": true}),
        )
        .await;
    assert_eq!(reply["result"]["cwd"], json!(project), "{reply}");
    assert!(reply["result"].get("wide").is_none(), "{reply}");
}
