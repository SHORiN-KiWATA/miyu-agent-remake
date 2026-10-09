//! 后台运行关着（施工 T-1 上，设计 `30-插件框架.md` 第三节第 7 条，`docs/blueprint/tools/shell.md`「后台」第 1 条）：真核心、
//! 真的基础系统走一遍。预设关了「后台运行」，工具面上 `shell` 没有放到后台那一项、没有 `jobs`，别的参数字节不动，快照记下
//! `foreground`；她硬写了放到后台的，回「这里不能放到后台」，不派任务。开着的照旧。

use serde_json::{Value, json};

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Body, ToolStatus};
use miyu_kernel::request::Request;
use miyu_policy::Snapshot;
use miyu_session::testkit::{Play, Script};
use miyu_store::blob::Blobs;
use miyu_tool::Catalog;

use crate::support::venues::configured_core;
use crate::support::*;

/// 关了后台运行的预设。
const QUIET: &str = "[preset]\nname = { en = \"Quiet\" }\n\n[features]\nbackground = false\n";

/// 照 `script` 起来、工具目录是出厂的基础系统的核心，造一个用预设 `preset` 的会话、说一句，交回会话编号。
async fn session(home: &Home, script: &Script, preset: &str) -> String {
    let tools = Catalog::new(miyu_basesystem::tools(&default_resources()).expect("出厂的工具"))
        .expect("合写法");
    let mut client = Client::connect(configured_core(home, script, tools));
    client.hello().await;
    let reply = client
        .call(
            "c1",
            "session.create",
            json!({"cwd": "~", "persona": "engineer", "preset": preset}),
        )
        .await;
    let session = reply["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("没造出来：{reply}"))
        .to_string();
    // 完全放开：测试的核心没有沙盒，工作区那一档每条命令都要问人。
    let reply = client
        .call(
            "p1",
            "session.set_permission_level",
            json!({"session": session, "level": "full"}),
        )
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
    client.say("s1", &session, "hi").await;
    home.until_turns(&session, 1).await;
    session
}

/// 第一次请求里 `name` 那件工具的参数格式。
fn parameters(request: &Request, name: &str) -> Option<Value> {
    request
        .tools
        .iter()
        .find(|tool| tool.name == name)
        .map(|tool| serde_json::from_str(tool.parameters.get()).expect("是 JSON"))
}

/// 会话的策略快照。
fn snapshot(home: &Home, session: &str) -> Snapshot {
    let Body::SessionCreated(created) = &home.log(session)[0].body else {
        panic!("第 1 条应该是造会话");
    };
    let bytes = Blobs::new(home.root.blobs(&alice()))
        .get(&created.policy)
        .expect("快照在 blob 里");
    Snapshot::from_bytes(&bytes).expect("读得懂")
}

#[tokio::test]
async fn with_background_off_the_shell_has_no_background_and_refuses_it() {
    let home = Home::new();
    home.write("home/alice/presets/quiet.toml", QUIET);
    let script = Script::new([
        Play::calls(&[(
            "shell",
            r#"{"command":"echo hi","description":"Say hi","run_in_background":true}"#,
        )]),
        Play::Says("好。"),
    ]);
    let quiet = session(&home, &script, "quiet").await;
    let requests = script.requests();
    let request = &requests[0].1;
    let shell = parameters(request, "shell").expect("有 shell");
    assert!(
        shell["properties"].get("run_in_background").is_none(),
        "{shell}"
    );
    assert!(parameters(request, "jobs").is_none(), "没有 jobs");
    assert!(snapshot(&home, &quiet).foreground, "快照记下");
    let refused = home
        .log(&quiet)
        .into_iter()
        .find_map(|event| match event.body {
            Body::ToolResult(result) => Some(result),
            _ => None,
        })
        .expect("有结果");
    assert_eq!(refused.status, ToolStatus::Error, "出错");
    let said: String = refused
        .blocks
        .iter()
        .map(|block| match block {
            Block::Text(Text { text }) => text.clone(),
            _ => String::new(),
        })
        .collect();
    assert!(
        said.starts_with("Running in the background is not available here."),
        "{said}"
    );
    assert!(
        refused.effects.is_empty(),
        "不派任务：{:?}",
        refused.effects
    );

    // 开着的照旧：同一件 `shell` 只差那一项。
    let other = Home::new();
    let full = Script::new([Play::Says("嗯。")]);
    let on = session(&other, &full, "full").await;
    let request = &full.requests()[0].1;
    let mut whole = parameters(request, "shell").expect("有 shell");
    assert!(
        whole["properties"].get("run_in_background").is_some(),
        "{whole}"
    );
    assert!(parameters(request, "jobs").is_some(), "有 jobs");
    assert!(!snapshot(&other, &on).foreground);
    whole["properties"]
        .as_object_mut()
        .expect("是对象")
        .remove("run_in_background");
    assert_eq!(whole, shell, "别的参数不动");
}
