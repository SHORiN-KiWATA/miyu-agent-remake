//! 预设（施工 P-2 上，`docs/blueprint/presets.md`）：真核心走一遍。造会话照「开会话时指定、个人设置、系统配置」找预设，指着
//! 没有的、写错的不造；人格照「指定、`persona.default`」找，预设不再管（施工 P-4 上）；`session.created`、会话列表、`subscribe` 带上预设；
//! `venue.session` 带预设；`preset.list`、`preset.get`；`check` 查预设。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::venues::{BINDINGS, configured_core};
use crate::support::*;

/// 管理员（测试里是 alice）家目录里的预设 `id`。
fn mine(home: &Home, id: &str, text: &str) {
    home.write(&format!("home/alice/presets/{id}.toml"), text);
}

/// 读系统配置、个人设置的核心（`support::venues`）。
fn configured(home: &Home, script: &Script) -> Arc<Core> {
    configured_core(home, script, Catalog::default())
}

async fn connected(core: Arc<Core>) -> Client {
    let mut client = Client::connect(core);
    client.hello().await;
    client
}

async fn create(client: &mut Client, id: &str, params: Value) -> Value {
    client.call(id, "session.create", params).await
}

/// 造出来的会话编号。
fn made(reply: &Value) -> String {
    reply["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("没造出来：{reply}"))
        .to_string()
}

/// 会话日志里的 `session.created`。
fn created(home: &Home, session: &str) -> miyu_kernel::event::SessionCreated {
    home.log(session)
        .into_iter()
        .find_map(|event| match event.body {
            miyu_kernel::event::Body::SessionCreated(created) => Some(created),
            _ => None,
        })
        .expect("有造会话那一条")
}

#[tokio::test]
async fn without_a_preset_the_default_is_used_personal_over_system() {
    let home = Home::new();
    mine(&home, "mine", "[preset]\nunlisted = \"off\"\n");
    let script = Script::new([]);
    // 都没写：出厂的功能全开。
    let mut client = connected(configured(&home, &script)).await;
    let first = made(&create(&mut client, "c1", json!({"cwd": "~"})).await);
    // 系统配置写了 dev。
    home.write("system/config.toml", "[preset]\ndefault = \"dev\"\n");
    let mut client = connected(configured(&home, &script)).await;
    let second = made(&create(&mut client, "c2", json!({"cwd": "~"})).await);
    // 个人设置写了 mine，压着系统配置。
    home.write("home/alice/settings.toml", "[preset]\ndefault = \"mine\"\n");
    let mut client = connected(configured(&home, &script)).await;
    let third = made(&create(&mut client, "c3", json!({"cwd": "~"})).await);
    // 指定的压着默认。
    let fourth = made(&create(&mut client, "c4", json!({"cwd": "~", "preset": "full"})).await);
    let presets: Vec<Option<String>> = [&first, &second, &third, &fourth]
        .iter()
        .map(|session| created(&home, session).preset)
        .collect();
    assert_eq!(
        presets,
        [
            Some("full".to_string()),
            Some("dev".to_string()),
            Some("mine".to_string()),
            Some("full".to_string())
        ]
    );
}

#[tokio::test]
async fn the_persona_comes_from_the_request_then_the_default_and_never_the_preset() {
    let home = Home::new();
    home.write(
        "home/alice/personas/miyu/prompts/persona.md",
        "You are Miyu.\n",
    );
    home.write("system/config.toml", "[persona]\ndefault = \"miyu\"\n");
    mine(&home, "chat", "[preset]\nname = { en = \"Chat\" }\n");
    let script = Script::new([]);
    let mut client = connected(configured(&home, &script)).await;
    // 旧文件里写着默认人格的，当没写（施工 P-4 上，2026-10-08 项目主人：只去掉预设的「默认人格」）。
    mine(&home, "ghost", "[preset]\ndefault_persona = \"nobody\"\n");
    let cases = [
        (
            json!({"cwd": "~", "preset": "dev"}),
            Some("miyu"),
            "预设不管人格，照 persona.default",
        ),
        (
            json!({"cwd": "~", "preset": "chat"}),
            Some("miyu"),
            "照 persona.default",
        ),
        (
            json!({"cwd": "~", "preset": "ghost"}),
            Some("miyu"),
            "旧文件里的默认人格当没写",
        ),
        (
            json!({"cwd": "~", "preset": "dev", "persona": "none"}),
            Some("none"),
            "指定的压着默认的",
        ),
        (
            json!({"cwd": "~", "preset": "dev", "persona": null}),
            None,
            "null 明着无人格",
        ),
    ];
    for (index, (params, persona, why)) in cases.into_iter().enumerate() {
        let session = made(&create(&mut client, &format!("c{index}"), params).await);
        assert_eq!(
            created(&home, &session).persona.as_deref(),
            persona,
            "{why}"
        );
    }
}

#[tokio::test]
async fn missing_bad_and_broken_presets_are_refused_and_nothing_is_made() {
    let home = Home::new();
    mine(&home, "broken", "[preset]\n\nunlisted = \"maybe\"\n");
    let script = Script::new([]);
    let mut client = connected(configured(&home, &script)).await;
    let reply = create(&mut client, "c1", json!({"cwd": "~", "preset": "nobody"})).await;
    assert_eq!(reason(&reply), Some("unknown_preset"), "{reply}");
    let reply = create(&mut client, "c2", json!({"cwd": "~", "preset": "../x"})).await;
    assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
    let reply = create(&mut client, "c3", json!({"cwd": "~", "preset": "broken"})).await;
    assert_eq!(reason(&reply), Some("preset_invalid"), "{reply}");
    assert_eq!(
        reply["error"]["data"]["problem"],
        "home broken.toml:3: preset.unlisted must be on or off"
    );
    // 默认预设指着没有的：照样不悄悄换成别的（Y12）。
    home.write("system/config.toml", "[preset]\ndefault = \"nobody\"\n");
    let mut client = connected(configured(&home, &script)).await;
    let reply = create(&mut client, "c4", json!({"cwd": "~"})).await;
    assert_eq!(reason(&reply), Some("unknown_preset"), "{reply}");
    let listed = client.call("l1", "session.list", json!({})).await;
    assert_eq!(listed["result"]["sessions"], json!([]), "什么都没造");
}

#[tokio::test]
async fn every_view_names_the_preset_and_older_logs_name_none() {
    let home = Home::new();
    let script = Script::new([Play::Says("嗯。")]);
    let mut client = connected(configured(&home, &script)).await;
    let dev = made(&create(&mut client, "c1", json!({"cwd": "~", "preset": "dev"})).await);
    let full = made(&create(&mut client, "c2", json!({"cwd": "~"})).await);
    let listed = client.call("l1", "session.list", json!({})).await;
    let presets: Vec<(String, String)> = listed["result"]["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| {
            (
                item["session"].as_str().unwrap().to_string(),
                item["preset"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect();
    assert_eq!(
        presets,
        [
            (full.clone(), "full".to_string()),
            (dev.clone(), "dev".to_string())
        ]
    );
    let subscribed = client.subscribe("w1", &dev).await;
    assert_eq!(subscribed["result"]["preset"], "dev", "{subscribed}");
    client.say("s1", &dev, "hi").await;
    home.until_turns(&dev, 1).await;
    // 把造会话那一条改回以前的样子：去掉 preset。
    let first = home
        .root
        .path()
        .join("home/alice/sessions")
        .join(&dev)
        .join("000000000001.jsonl");
    let text = std::fs::read_to_string(&first).unwrap();
    let old = text.replacen(",\"preset\":\"dev\"", "", 1);
    assert_ne!(old, text, "写过 preset");
    std::fs::write(&first, old).unwrap();
    let mut client = connected(configured(&home, &Script::new([]))).await;
    let subscribed = client.subscribe("w2", &dev).await;
    assert!(subscribed["result"].get("preset").is_none(), "{subscribed}");
    let listed = client.call("l2", "session.list", json!({})).await;
    let old = listed["result"]["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["session"] == dev.as_str())
        .cloned()
        .expect("列得出");
    assert!(old.get("preset").is_none(), "{old}");
}

#[tokio::test]
async fn a_venue_session_is_made_with_the_given_preset_and_found_again_without_it() {
    let home = Home::new();
    home.write("system/config.toml", BINDINGS);
    let script = Script::new([]);
    let mut client = connected(configured(&home, &script)).await;
    let private = json!({"venue": "qq:private:10001", "kind": "private", "peer": "qq:10001", "preset": "dev"});
    let made = client.call("v1", "venue.session", private).await;
    let session = made["result"]["session"]
        .as_str()
        .expect("造出来了")
        .to_string();
    assert_eq!(created(&home, &session).preset.as_deref(), Some("dev"));
    assert_eq!(
        created(&home, &session).persona,
        None,
        "预设不管人格，没设默认人格的无人格（施工 P-4 上）"
    );
    let again = json!({"venue": "qq:private:10001", "kind": "private", "peer": "qq:10001", "preset": "nobody"});
    let found = client.call("v2", "venue.session", again).await;
    assert_eq!(
        found["result"],
        json!({"session": session, "created": false}),
        "找回时不看预设"
    );
    let other = json!({"venue": "qq:private:10001x", "kind": "private", "peer": "qq:10001", "preset": "nobody"});
    let reply = client.call("v3", "venue.session", other).await;
    assert_eq!(reason(&reply), Some("unknown_preset"), "{reply}");
}

#[tokio::test]
async fn presets_are_listed_and_read_by_layer() {
    let home = Home::new();
    home.write(
        "system/presets/dev.toml",
        "[preset]\nname = { zh = \"大家的开发\" }\n\n[tools]\ntrash = false\n",
    );
    mine(&home, "dev", "[software]\nmemory = true\n");
    mine(&home, "broken", "[colors]\n");
    let mut client = connected(configured(&home, &Script::new([]))).await;
    let listed = client.call("p1", "preset.list", json!({})).await;
    assert_eq!(
        listed["result"]["presets"],
        json!([
            {"preset": "broken", "problem": "不认识的表 [colors]：预设文件里只能有 [preset]、[software]、[tools]", "line": 1},
            {"preset": "dev", "name": "大家的开发", "summary": "只开写代码必需的：基础系统、联网、长期目标"},
            {"preset": "full", "name": "功能全开", "summary": "装了的软件全部打开，以后新装的也打开"},
        ]),
        "照连接的语言挑"
    );
    let got = client
        .call("g1", "preset.get", json!({"preset": "dev"}))
        .await;
    let got = &got["result"];
    assert_eq!(got["name"], "大家的开发", "照连接的语言挑一句");
    assert!(got.get("layers").is_none(), "来自哪几层不给（施工 P-3 补）");
    assert!(
        got.get("default_persona").is_none(),
        "预设没有默认人格这一格（施工 P-4 上）"
    );
    assert_eq!(got["unlisted"], "off");
    let software: Vec<(&str, &str, bool)> = got["software"]
        .as_array()
        .expect("是一个个软件")
        .iter()
        .map(|one| {
            (
                one["id"].as_str().unwrap_or_default(),
                one["name"].as_str().unwrap_or_default(),
                one["on"].as_bool().unwrap_or_default(),
            )
        })
        .collect();
    assert_eq!(
        software,
        [
            ("basesystem", "基础系统", true),
            ("net", "联网", true),
            ("goal", "长期目标", true),
            ("memory", "人格记忆", true),
            ("roleplay", "角色扮演", false),
        ],
        "内置的照固定的先后、照连接的语言写名字（施工 P-3 补）"
    );
    assert_eq!(got["software"][4]["installed"], true, "角色扮演一直装着");
    assert_eq!(got["software"][4]["summary"], "照人格的设定演下去，不出戏");
    assert_eq!(got["tools"], json!({"trash": false}));
    let full = client
        .call("g2", "preset.get", json!({"preset": "full"}))
        .await;
    assert_eq!(full["result"]["unlisted"], "on");
    assert!(full["result"].get("default_persona").is_none());
    let reply = client
        .call("g3", "preset.get", json!({"preset": "nobody"}))
        .await;
    assert_eq!(reason(&reply), Some("unknown_preset"), "{reply}");
    let reply = client
        .call("g4", "preset.get", json!({"preset": "broken"}))
        .await;
    assert_eq!(reason(&reply), Some("preset_invalid"), "{reply}");
}

#[tokio::test]
async fn check_reads_presets_from_disk() {
    let home = Home::new();
    let mut client = connected(configured(&home, &Script::new([]))).await;
    mine(&home, "bad", "[tools]\nshell = true\n");
    mine(&home, "fine", "");
    let reply = client.call("c1", "check", json!({})).await;
    let presets: Vec<&Value> = reply["result"]["problems"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|problem| problem["kind"] == "preset")
        .collect();
    assert_eq!(
        presets,
        [&json!({
            "kind": "preset",
            "file": "home/alice/presets/bad.toml",
            "code": "not_false",
            "level": "error",
            "line": 2,
            "message": "tools.shell 只能写 false：预设里只能关掉单件工具",
        })],
        "{reply}"
    );
    let file = home.root.path().join("home/alice/presets/fine.toml");
    let reply = client
        .call("c2", "check", json!({"file": file.to_string_lossy()}))
        .await;
    assert_eq!(reply["result"], json!({"problems": []}), "{reply}");
    let file = home.root.path().join("home/alice/presets/bad.toml");
    let reply = client
        .call("c3", "check", json!({"file": file.to_string_lossy()}))
        .await;
    assert_eq!(
        reply["result"]["problems"][0]["code"], "not_false",
        "{reply}"
    );
    let file = home.root.path().join("home/alice/presets/gone.toml");
    let reply = client
        .call("c4", "check", json!({"file": file.to_string_lossy()}))
        .await;
    assert_eq!(
        reply["result"]["problems"][0]["code"], "unreadable",
        "{reply}"
    );
}
