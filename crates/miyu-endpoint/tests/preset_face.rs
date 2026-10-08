//! 照预设挑（施工 P-2 中，`docs/blueprint/presets.md`「照预设挑」）：真核心走一遍。工具面只有开着的包里的、没被单独关掉的；
//! 记忆没开的会话范围是 `off`；角色扮演没开的没有角色扮演提示和风格锁；装了没开的软件写一行进 system；功能全开的和以前一样；
//! `preset.get` 写出没装的。

mod support;

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_kernel::event::Body;
use miyu_kernel::request::Request;
use miyu_kernel::tool::Access;
use miyu_policy::Snapshot;
use miyu_session::testkit::{Play, Script};
use miyu_store::blob::Blobs;
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use support::venues::configured_core;
use support::*;

/// 基础系统两件、记忆三件，照包登记。
fn catalog() -> Catalog {
    let fake = |name: &str| -> Arc<dyn Tool> { Fake::new(name, Access::Read, Act::Echo) };
    Catalog::in_packages([
        ("basesystem", vec![fake("read"), fake("shell")]),
        (
            "memory",
            vec![fake("remember"), fake("forget"), fake("memory_search")],
        ),
    ])
    .expect("合写法")
}

fn configured(home: &Home, script: &Script) -> Arc<Core> {
    configured_core(home, script, catalog())
}

async fn connected(core: Arc<Core>) -> Client {
    let mut client = Client::connect(core);
    client.hello().await;
    client
}

/// 照 `params` 造一个会话、说一句，交回会话编号和它发出的那一次请求。
async fn first_request(home: &Home, params: Value) -> (String, Request) {
    let script = Script::new([Play::Says("嗯。")]);
    let mut client = connected(configured(home, &script)).await;
    // 每次一个新的命令编号：同一个数据根上再起的核心认得以前的编号，会交回上一次造的那个。
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let id = format!(
        "c{}",
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    let reply = client.call(&id, "session.create", params).await;
    let session = reply["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("没造出来：{reply}"))
        .to_string();
    client.say("s1", &session, "hi").await;
    home.until_turns(&session, 1).await;
    let (_, request) = script.requests().into_iter().next().expect("发了请求");
    (session, request)
}

fn tool_names(request: &Request) -> Vec<&str> {
    request
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect()
}

/// 会话的策略快照。
fn snapshot(home: &Home, session: &str) -> Snapshot {
    let log = home.log(session);
    let Body::SessionCreated(created) = &log[0].body else {
        panic!("第 1 条应该是造会话");
    };
    let bytes = Blobs::new(home.root.blobs(&alice()))
        .get(&created.policy)
        .expect("快照在 blob 里");
    Snapshot::from_bytes(&bytes).expect("读得懂")
}

const OFF_LINE: &str = "Installed but off in this session's preset:";

#[tokio::test]
async fn everything_on_keeps_every_tool_and_no_line() {
    let home = Home::new();
    let (session, request) = first_request(&home, json!({"cwd": "~"})).await;
    assert_eq!(
        tool_names(&request),
        ["forget", "memory_search", "read", "remember", "shell"]
    );
    assert!(!request.system.contains(OFF_LINE), "{}", request.system);
    let snapshot = snapshot(&home, &session);
    assert_eq!(snapshot.memory.as_deref(), Some("persona"));
    let pin = snapshot.preset.expect("记了预设");
    assert_eq!((pin.id.as_str(), pin.off.len()), ("full", 0));
}

#[tokio::test]
async fn the_dev_preset_drops_memory_and_says_so() {
    let home = Home::new();
    let (session, request) = first_request(&home, json!({"cwd": "~", "preset": "dev"})).await;
    assert_eq!(tool_names(&request), ["read", "shell"], "记忆没开");
    assert!(
        request
            .system
            .ends_with("Installed but off in this session's preset: memory."),
        "角色扮演不进这一行：{}",
        request.system
    );
    let snapshot = snapshot(&home, &session);
    assert_eq!(
        snapshot.memory.as_deref(),
        Some("off"),
        "记忆没开的范围是 off"
    );
    let pin = snapshot.preset.expect("记了预设");
    assert_eq!(pin.id, "dev");
    assert_eq!(pin.off, ["memory", "roleplay"]);
    // 开会话时要了记忆也没用：开不开归预设。
    let (session, _) = first_request(
        &home,
        json!({"cwd": "~", "preset": "dev", "memory": "session"}),
    )
    .await;
    assert_eq!(memory_of(&home, &session).as_deref(), Some("off"));
}

fn memory_of(home: &Home, session: &str) -> Option<String> {
    snapshot(home, session).memory
}

#[tokio::test]
async fn a_tool_turned_off_alone_is_gone_and_nothing_is_said() {
    let home = Home::new();
    home.write(
        "home/alice/presets/careful.toml",
        "[preset]\nname = { en = \"Careful\" }\n\n[tools]\nshell = false\n",
    );
    let (_, request) = first_request(&home, json!({"cwd": "~", "preset": "careful"})).await;
    assert_eq!(
        tool_names(&request),
        ["forget", "memory_search", "read", "remember"]
    );
    assert!(
        !request.system.contains(OFF_LINE),
        "包都开着：{}",
        request.system
    );
}

#[tokio::test]
async fn roleplay_off_drops_the_reminder_and_the_style_lock_and_the_line_comes_before_the_lock() {
    let home = Home::new();
    home.write(
        "home/alice/personas/miyu/prompts/persona.md",
        "You are Miyu.\n",
    );
    home.write(
        "home/alice/personas/miyu/prompts/reminders.md",
        "Stay soft.\n",
    );
    home.write(
        "home/alice/presets/quiet.toml",
        "[preset]\nname = { en = \"Quiet\" }\n\n[software]\nmemory = false\n",
    );
    let lock = std::fs::read_to_string(default_resources().join("core/style-lock.txt"))
        .expect("读得出风格锁");
    let reminded = |request: &Request| format!("{request:?}").contains("Stay soft.");
    // 角色扮演开着、记忆关着：没开的那一行在风格锁前面。
    let (_, quiet) = first_request(
        &home,
        json!({"cwd": "~", "persona": "miyu", "preset": "quiet"}),
    )
    .await;
    assert!(reminded(&quiet), "角色扮演开着的有提示");
    assert!(
        quiet.system.ends_with(&format!(
            "Installed but off in this session's preset: memory.\n\n{}",
            lock.trim_end()
        )),
        "{}",
        quiet.system
    );
    // 开发预设：角色扮演也关着。
    let (session, dev) = first_request(
        &home,
        json!({"cwd": "~", "persona": "miyu", "preset": "dev"}),
    )
    .await;
    assert!(!reminded(&dev), "没有角色扮演提示：{dev:?}");
    assert!(!dev.system.contains(lock.trim_end()), "没有风格锁");
    assert!(snapshot(&home, &session).reminder.is_none());
}

#[tokio::test]
async fn preset_get_names_software_that_is_not_installed() {
    let home = Home::new();
    let mut client = connected(configured(&home, &Script::new([]))).await;
    let dev = client
        .call("g1", "preset.get", json!({"preset": "dev"}))
        .await;
    assert_eq!(dev["result"]["missing"], json!(["goal", "net"]), "{dev}");
    assert_eq!(
        dev["result"]["switches"],
        json!({"basesystem": true, "memory": false, "roleplay": false}),
        "装了的每一个都有开关（施工 P-2 补）"
    );
    let full = client
        .call("g2", "preset.get", json!({"preset": "full"}))
        .await;
    assert_eq!(full["result"]["missing"], json!([]));
    assert_eq!(
        full["result"]["switches"],
        json!({"basesystem": true, "memory": true, "roleplay": true})
    );
}

#[tokio::test]
async fn a_package_turned_off_takes_all_its_tools_and_is_named() {
    let home = Home::new();
    home.write(
        "home/alice/presets/nobase.toml",
        "[preset]\nname = { en = \"No base\" }\n\n[software]\nbasesystem = false\n",
    );
    let (_, request) = first_request(&home, json!({"cwd": "~", "preset": "nobase"})).await;
    assert_eq!(
        tool_names(&request),
        ["forget", "memory_search", "remember"],
        "基础系统的两件都没了，记忆照开"
    );
    assert!(
        request
            .system
            .ends_with("Installed but off in this session's preset: basesystem."),
        "{}",
        request.system
    );
}
