//! 照预设挑（施工 P-2 中，`docs/blueprint/presets.md`「照预设挑」）：真核心走一遍。工具面只有开着的包里的、没被单独关掉的；
//! 记忆没开的会话范围是 `off`；角色扮演没开的没有角色扮演提示和风格锁；装了没开的软件写一行进 system；功能全开的和以前一样；
//! `preset.get` 写出没装的。

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

use crate::support::venues::configured_core;
use crate::support::*;

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
    let (session, request) = first_request(&home, json!({"cwd": "~", "persona": "engineer"})).await;
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
    // 施工 F-3 上起记的是功能的编号。出厂的接入QQ 的功能 qq 在不在看测试程序旁边有没有 `miyu-onebot`（桥的测试会链一个过去），这里不比它；装了、没开、没有工具的包快照照记，见下一条。
    let off: Vec<&String> = pin.off.iter().filter(|id| *id != "qq").collect();
    assert_eq!(off, ["memory", "roleplay"]);
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

/// 装了没开的那一行只列有工具的包（施工 O-18，2026-10-08 主会话定，`presets.md`「照预设挑」）：没有工具的 `process` 包她用
/// 不上，桥这种又是核心拉起、不随会话的预设，写「这次的预设里关着」是假话。快照照旧记它没开。
#[tokio::test]
async fn software_without_tools_is_pinned_off_but_left_out_of_the_line() {
    let home = Home::new();
    // 程序要在测试程序旁边：不在的当没装（施工 F-6 上）。
    let program = crate::support::extensions::Program::new();
    home.write(
        "home/alice/packages/quiet.toml",
        &format!(
            "[package]\nkind = \"process\"\nprotocol = [1, 1]\nname = {{ en = \"Quiet\" }}\n\n[command]\nname = \"quiet\"\nprogram = \"{}\"\nabout = {{ en = \"Q\" }}\n\n[process]\nargs = []\n",
            program.name()
        ),
    );
    home.write(
        "home/alice/presets/lean.toml",
        "[preset]\nname = { en = \"Lean\" }\n\n[software]\nquiet = false\nmemory = false\n",
    );
    let (session, request) = first_request(
        &home,
        json!({"cwd": "~", "persona": "engineer", "preset": "lean"}),
    )
    .await;
    assert!(
        request
            .system
            .ends_with("Installed but off in this session's preset: memory."),
        "有工具的照列，没有工具的不列：{}",
        request.system
    );
    let pin = snapshot(&home, &session).preset.expect("记了预设");
    assert_eq!(pin.off, ["memory", "quiet"], "快照照旧记全部没开的");
}

#[tokio::test]
async fn a_tool_turned_off_alone_is_gone_and_nothing_is_said() {
    let home = Home::new();
    home.write(
        "home/alice/presets/careful.toml",
        "[preset]\nname = { en = \"Careful\" }\n\n[tools]\nshell = false\n",
    );
    let (_, request) = first_request(
        &home,
        json!({"cwd": "~", "persona": "engineer", "preset": "careful"}),
    )
    .await;
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

/// `preset.get` 照功能列（施工 F-3 下，设计 30 第四节）：装了的每个功能都有开关，各带归它的工具（显示名照给人看的字，开不开
/// 照功能和 `[tools]`）；写了没装的标着没装，接在后面。
#[tokio::test]
async fn preset_get_lists_features_with_their_tools() {
    let home = Home::new();
    home.write(
        "home/alice/presets/nosh.toml",
        "[preset]\nname = { en = \"No sh\" }\n\n[features]\nqq = false\n\n[tools]\nforget = false\n",
    );
    let mut client = connected(configured(&home, &Script::new([]))).await;
    let dev = client
        .call("g1", "preset.get", json!({"preset": "dev"}))
        .await;
    let listed = switches(&dev);
    assert_eq!(
        listed[..2],
        [("files", true, true), ("commands", true, true)],
        "{dev}"
    );
    assert!(listed.contains(&("memory", false, true)));
    assert_eq!(
        listed.last(),
        Some(&("goal", true, false)),
        "写了没装的接在后面"
    );
    let files = feature(&dev, "files");
    assert_eq!(
        files["tools"],
        json!([{"name": "read", "label": "读取", "on": true}]),
        "目录里只有 read 归文件读写"
    );
    assert_eq!(
        feature(&dev, "memory")["tools"],
        json!([
            {"name": "forget", "label": "忘掉", "on": false},
            {"name": "memory_search", "label": "翻记忆", "on": false},
            {"name": "remember", "label": "记住", "on": false},
        ]),
        "功能关着的工具都是关着"
    );
    let nosh = client
        .call("g2", "preset.get", json!({"preset": "nosh"}))
        .await;
    assert_eq!(
        feature(&nosh, "memory")["tools"],
        json!([
            {"name": "forget", "label": "忘掉", "on": false},
            {"name": "memory_search", "label": "翻记忆", "on": true},
            {"name": "remember", "label": "记住", "on": true},
        ]),
        "[tools] 单件关掉的"
    );
    // 写在 [features] 里的：装了的照装了的列，没装的（程序不在的也是）照「写了没装」接在后面，都只列一遍。出厂的接入QQ 的功能 qq 在不在看测试程序旁边有没有 `miyu-onebot`（桥的测试会链一个过去），这里不比它装没装。
    assert_eq!(feature(&nosh, "qq")["on"], false);
    assert_eq!(
        switches(&nosh)
            .iter()
            .filter(|(id, _, _)| *id == "qq")
            .count(),
        1,
        "只列一遍"
    );
    assert_eq!(feature(&nosh, "commands")["tools"][0]["label"], "执行命令");
}

/// 回应里编号是 `id` 的那个功能。
fn feature<'a>(reply: &'a serde_json::Value, id: &str) -> &'a serde_json::Value {
    reply["result"]["features"]
        .as_array()
        .expect("是一个个功能")
        .iter()
        .find(|one| one["id"] == id)
        .unwrap_or_else(|| panic!("没有 {id}：{reply}"))
}

/// `preset.get` 的功能：编号、开不开、装没装（施工 F-3 下）。
fn switches(reply: &serde_json::Value) -> Vec<(&str, bool, bool)> {
    reply["result"]["features"]
        .as_array()
        .expect("是一个个功能")
        .iter()
        .map(|one| {
            (
                one["id"].as_str().unwrap_or_default(),
                one["on"].as_bool().unwrap_or_default(),
                one["installed"].as_bool().unwrap_or_default(),
            )
        })
        .collect()
}

#[tokio::test]
async fn a_package_turned_off_takes_all_its_tools_and_is_named() {
    let home = Home::new();
    home.write(
        "home/alice/presets/nobase.toml",
        "[preset]\nname = { en = \"No base\" }\n\n[software]\nbasesystem = false\n",
    );
    let (_, request) = first_request(
        &home,
        json!({"cwd": "~", "persona": "engineer", "preset": "nobase"}),
    )
    .await;
    assert_eq!(
        tool_names(&request),
        ["forget", "memory_search", "remember"],
        "基础系统的两件都没了，记忆照开"
    );
    // 施工 F-3 上起那一行写功能的编号：关掉的包，写它下面有工具的功能。
    assert!(
        request
            .system
            .ends_with("Installed but off in this session's preset: commands, files."),
        "{}",
        request.system
    );
}

/// 同 `configured`，但核心认的清单照 `packages`（施工 F-3 上：没装哪个包就是没有它的清单）。
fn configured_with(home: &Home, script: &Script, without: &[&str]) -> Arc<Core> {
    let items = [
        miyu_endpoint::settings::UiSettings::ITEMS,
        miyu_endpoint::settings::PersonaSettings::ITEMS,
        miyu_endpoint::settings::PresetSettings::ITEMS,
        miyu_endpoint::settings::PermissionSettings::ITEMS,
        miyu_endpoint::settings::EXTERNAL_BINDINGS,
    ]
    .concat();
    let config = miyu_endpoint::config::Config::load(
        &home.root,
        &alice(),
        None,
        items,
        miyu_endpoint::config::Environment::of(&[]),
    );
    let resources = miyu_store::resources::ResourceRoot::at(default_resources());
    let mut found = miyu_endpoint::packages::load(&resources, &home.root, &alice());
    found.retain(|one| !without.contains(&one.id.as_str()));
    Arc::new(
        home.core_full(script, catalog(), None, TOKEN)
            .with_config(config)
            .with_packages(found),
    )
}

/// 照 `params` 造一个会话、说一句：核心照 `configured_with` 造，交回会话编号和那一次请求。
async fn first_request_without(home: &Home, without: &[&str], params: Value) -> (String, Request) {
    let script = Script::new([Play::Says("嗯。")]);
    let mut client = connected(configured_with(home, &script, without)).await;
    let reply = client.call("c1", "session.create", params).await;
    let session = reply["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("没造出来：{reply}"))
        .to_string();
    client.say("s1", &session, "hi").await;
    home.until_turns(&session, 1).await;
    let (_, request) = script.requests().into_iter().next().expect("发了请求");
    (session, request)
}

/// 预设照功能开关（施工 F-3 上，设计 30 第四节）：`[features]` 关掉运行命令，`shell` 不给，那一行写功能的编号；文件读写照开。
#[tokio::test]
async fn a_feature_switched_off_takes_its_tools_and_is_named() {
    let home = Home::new();
    home.write(
        "home/alice/presets/noshell.toml",
        "[preset]\nname = { en = \"No shell\" }\n\n[features]\ncommands = false\n",
    );
    let (session, request) = first_request(
        &home,
        json!({"cwd": "~", "persona": "engineer", "preset": "noshell"}),
    )
    .await;
    assert_eq!(
        tool_names(&request),
        ["forget", "memory_search", "read", "remember"]
    );
    assert!(
        request
            .system
            .ends_with("Installed but off in this session's preset: commands."),
        "{}",
        request.system
    );
    let pin = snapshot(&home, &session).preset.expect("记了预设");
    assert_eq!(pin.off, ["commands"]);
}

/// 没装人格记忆（没有它的清单，施工 F-3 上）：全部功能的预设、带人格的会话，记忆的范围也是 `off`，三件工具不给，那一行也不写
/// （没装的不是「装了没开」）。
#[tokio::test]
async fn memory_not_installed_is_off_and_not_named() {
    let home = Home::new();
    let (session, request) = first_request_without(
        &home,
        &["memory"],
        json!({"cwd": "~", "persona": "engineer", "preset": "full"}),
    )
    .await;
    assert_eq!(memory_of(&home, &session).as_deref(), Some("off"));
    assert_eq!(tool_names(&request), ["read", "shell"]);
    assert!(!request.system.contains(OFF_LINE), "{}", request.system);
}

/// 交进来的清单里没有人格记忆（`Core::with_packages`，核心起来时的那一份）：`memory.*` 说没装（施工 R-10，`memory.md` 第十一条
/// 第 1 款）。
#[tokio::test]
async fn memory_not_in_the_given_list_says_not_installed() {
    let home = Home::new();
    let mut client = connected(configured_with(&home, &Script::new([]), &["memory"])).await;
    let reply = client.call("m1", "memory.list", json!({})).await;
    assert_eq!(reason(&reply), Some("memory_not_installed"), "{reply}");
}

/// 没装人设遵循提醒（施工 F-3 上）：人格写了提醒短语，全部功能的预设里也没有提醒、没有风格锁。
#[tokio::test]
async fn reminders_not_installed_drop_the_reminder_and_the_lock() {
    let home = Home::new();
    home.write(
        "home/alice/personas/miyu/prompts/persona.md",
        "You are Miyu.\n",
    );
    home.write(
        "home/alice/personas/miyu/prompts/reminders.md",
        "Stay soft.\n",
    );
    let lock = std::fs::read_to_string(default_resources().join("core/style-lock.txt"))
        .expect("读得出风格锁");
    let (session, request) = first_request_without(
        &home,
        &["roleplay"],
        json!({"cwd": "~", "persona": "miyu", "preset": "full"}),
    )
    .await;
    assert!(
        !format!("{request:?}").contains("Stay soft."),
        "{request:?}"
    );
    assert!(!request.system.contains(lock.trim_end()), "没有风格锁");
    assert!(snapshot(&home, &session).reminder.is_none());
}
