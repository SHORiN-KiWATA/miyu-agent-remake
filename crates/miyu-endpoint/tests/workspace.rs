//! 工作目录太宽（施工 4-3 下，`11-权限与沙盒.md` 第四节）：头报来的是 `~`、系统的家目录、根目录，或者包含
//! 数据根、落在数据根里的，退回管理员的工作区 `home/<账号>/workspace/`；项目目录照旧。造会话、说话的回应
//! 说会话实际在哪个目录里干活（施工 4-5 下）。

mod support;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Value, json};

use miyu_kernel::template::escape;
use miyu_session::testkit::{Play, Script};
use support::{Client, Home, alice};

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

/// 头报来的工作目录是 `cwd`：她看到的请求里的字。
async fn seen(home: &Home, outside: &Outside, cwd: &str) -> String {
    let script = Script::new([Play::Says("好。")]);
    let mut client = Client::connect(home.core_at_home(&script, outside.0.join("home")));
    client.hello().await;
    let session = client.create("c1", cwd).await;
    client
        .call(
            "c2",
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
    // 接着说时换到了太宽的目录：回应里是退回的工作区。
    let reply = client
        .call(
            "c2",
            "session.send",
            json!({"session": session, "text": "hi", "cwd": "~"}),
        )
        .await;
    assert_eq!(reply["result"]["cwd"], json!(own), "{reply}");
    home.until_turns(&session, 1).await;
    // 不带目录的，是会话现在的。
    let reply = client
        .call(
            "c3",
            "session.send",
            json!({"session": session, "text": "hi"}),
        )
        .await;
    assert_eq!(reply["result"]["cwd"], json!(own), "{reply}");
    home.until_turns(&session, 2).await;
    // 造会话时就太宽的；同一个命令编号重发的，说的一样。
    for _ in 0..2 {
        let reply = client
            .call("c4", "session.create", json!({"cwd": "~"}))
            .await;
        assert_eq!(reply["result"]["cwd"], json!(own), "{reply}");
    }
}
