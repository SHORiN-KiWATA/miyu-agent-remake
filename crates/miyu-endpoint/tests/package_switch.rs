//! 软件包列表的状态、开关、图标、后台页，程序不在就当没装（施工 F-6 上，`docs/blueprint/package-pages.md`）：真核心走一遍。
//! 出厂的接入QQ、两个界面的程序不在测试程序旁边，是「程序不在」的样子；自己装的扩展用测试用的扩展，是「程序在」的样子。

use std::sync::Arc;

use serde_json::{Value, json};

use crate::support::extensions::{Program, core, core_with_settings, quick, until_state};
use crate::support::*;

/// 一个自己装的扩展：程序是 `program`；`extra` 接在 `[package]` 里，`tables` 接在最后。
fn relay(program: &str, extra: &str, tables: &str) -> String {
    format!(
        "[package]\nkind = \"process\"\nprotocol = [1, 1]\nname = {{ en = \"Relay\", zh = \"中转\" }}\n{extra}\n\n[command]\nname = \"relay\"\nprogram = \"{program}\"\nabout = {{ en = \"Relay\" }}\n\n[process]\nargs = [\"hello\", \"wait\"]\nstart = \"manual\"\n{tables}"
    )
}

/// 一个程序一定不在的扩展：带一个功能、一项配置。演「程序不在」的样子：不借出厂的接入QQ，桥的测试会往测试程序旁边链
/// `miyu-onebot`，借它的话先跑过桥的测试就红。
fn ghost(home: &Home) {
    home.write(
        "home/alice/packages/ghost.toml",
        "[package]\nkind = \"process\"\nprotocol = [1, 1]\nname = { en = \"Ghost\" }\n\n[command]\nname = \"ghost\"\nprogram = \"miyu-no-such-program-anywhere\"\nabout = { en = \"G\" }\n\n[process]\n\n[features.haunt]\nname = { en = \"Haunt\" }\n\n[settings.port]\ntype = \"int\"\nlayers = [\"system\"]\nname = { en = \"Port\" }\n",
    );
}

/// `package.list` 里编号是 `id` 的那一项。
async fn entry(client: &mut Client, id: &str) -> Value {
    let reply = client.call("l", "package.list", json!({})).await;
    reply["result"]["packages"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .find(|one| one["package"] == id)
        .cloned()
        .unwrap_or_else(|| panic!("没有 {id}：{reply}"))
}

/// 发一条只带 `package` 的方法，交回整条回应。
async fn switch(client: &mut Client, method: &str, id: &str) -> Value {
    client.call("s", method, json!({"package": id})).await
}

#[tokio::test]
async fn the_list_says_status_switch_icon_and_page() {
    let home = Home::new();
    let program = Program::new();
    home.write(
        "home/alice/packages/relay.toml",
        &relay(
            &program.name(),
            "icon = \"radio\"",
            "\n[page]\ndir = \"page\"\n",
        ),
    );
    home.write("home/alice/packages/relay/page/index.html", "<p>relay</p>");
    // 写了后台页、目录里没有 index.html 的不带 `page`。
    home.write(
        "home/alice/packages/bare.toml",
        &relay(&program.name(), "", "\n[page]\ndir = \"page\"\n")
            .replace("name = \"relay\"", "name = \"bare\""),
    );
    ghost(&home);
    let core = core(&home, quick());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;

    let relay = entry(&mut client, "relay").await;
    assert_eq!(
        (
            relay["status"].clone(),
            relay["enabled"].clone(),
            relay["icon"].clone(),
            relay["page"].clone()
        ),
        (json!("off"), json!(false), json!("radio"), json!(true)),
        "{relay}"
    );
    let bare = entry(&mut client, "bare").await;
    assert_eq!((bare.get("icon"), bare.get("page")), (None, None), "{bare}");
    let basesystem = entry(&mut client, "basesystem").await;
    assert_eq!(basesystem["status"], "ready");
    assert!(
        basesystem.get("enabled").is_none(),
        "必需的没有开关：{basesystem}"
    );
    let memory = entry(&mut client, "memory").await;
    assert_eq!(
        (memory["status"].clone(), memory["enabled"].clone()),
        (json!("ready"), json!(true))
    );
    let ghost = entry(&mut client, "ghost").await;
    assert_eq!(
        (ghost["status"].clone(), ghost["enabled"].clone()),
        (json!("program_missing"), json!(false)),
        "{ghost}"
    );
    let tui = entry(&mut client, "tui").await;
    assert_eq!(tui["status"], "program_missing");
    assert!(tui.get("enabled").is_none(), "界面没有开关：{tui}");
    core.stop_extensions().await;
}

#[tokio::test]
async fn the_switch_starts_and_stops_an_extension() {
    let home = Home::new();
    let program = Program::new();
    home.write(
        "home/alice/packages/relay.toml",
        &relay(&program.name(), "", ""),
    );
    let core = core(&home, quick());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;

    let on = switch(&mut client, "package.enable", "relay").await;
    assert_eq!(on["result"]["enabled"], true, "{on}");
    until_state(&mut client, "relay", |one| one["state"] == "running").await;
    assert_eq!(entry(&mut client, "relay").await["status"], "running");

    let off = switch(&mut client, "package.disable", "relay").await;
    assert_eq!(
        (
            off["result"]["enabled"].clone(),
            off["result"]["status"].clone()
        ),
        (json!(false), json!("off")),
        "{off}"
    );
    core.stop_extensions().await;
}

#[tokio::test]
async fn a_shipped_built_in_package_is_switched_off_by_removing_it() {
    let home = Home::new();
    let core = core(&home, quick());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;

    let off = switch(&mut client, "package.disable", "memory").await;
    assert_eq!(
        (
            off["result"]["enabled"].clone(),
            off["result"]["status"].clone(),
            off["result"]["removed"].clone()
        ),
        (json!(false), json!("off"), json!(true)),
        "{off}"
    );
    assert_eq!(entry(&mut client, "memory").await["removed"], true);
    // 关着的再关一次什么都不动。
    let again = switch(&mut client, "package.disable", "memory").await;
    assert_eq!(again["result"]["enabled"], false, "{again}");

    let on = switch(&mut client, "package.enable", "memory").await;
    assert_eq!(on["result"]["enabled"], true, "{on}");
    assert!(on["result"].get("removed").is_none(), "{on}");
}

#[tokio::test]
async fn switches_that_do_not_apply_are_refused() {
    let home = Home::new();
    ghost(&home);
    let core = core(&home, quick());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    for (method, id, reason) in [
        ("package.enable", "basesystem", "package_required"),
        ("package.disable", "basesystem", "package_required"),
        ("package.enable", "tui", "not_switchable"),
        ("package.disable", "web", "not_switchable"),
        ("package.enable", "ghost", "program_missing"),
        ("extension.enable", "ghost", "program_missing"),
        ("package.enable", "nothing", "unknown_package"),
    ] {
        let reply = switch(&mut client, method, id).await;
        assert_eq!(
            reply["error"]["data"]["reason"], reason,
            "{method} {id}：{reply}"
        );
    }
    assert_eq!(
        entry(&mut client, "ghost").await["enabled"],
        false,
        "程序不在的开不了"
    );
}

#[tokio::test]
async fn a_package_whose_program_is_missing_counts_as_not_installed() {
    let home = Home::new();
    ghost(&home);
    let core = core_with_settings(&home, quick());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let schema = client.call("c", "config.schema", json!({})).await;
    let keys: Vec<&str> = schema["result"]["items"]
        .as_array()
        .unwrap_or_else(|| panic!("{schema}"))
        .iter()
        .filter_map(|item| item["key"].as_str())
        .collect();
    assert!(!keys.contains(&"ghost.port"), "{keys:?}");
    assert!(
        keys.contains(&"tui.icons"),
        "界面的程序不在，配置项照旧在：{keys:?}"
    );
    let preset = client
        .call("p", "preset.get", json!({"preset": "full"}))
        .await;
    let installed: Vec<&str> = preset["result"]["features"]
        .as_array()
        .unwrap_or_else(|| panic!("{preset}"))
        .iter()
        .filter(|feature| feature["installed"] == true)
        .filter_map(|feature| feature["id"].as_str())
        .collect();
    assert!(!installed.contains(&"haunt"), "{installed:?}");
    core.stop_extensions().await;
}

/// 写了后台页、目录里没有 `index.html`（施工 F-6 上，`package-pages.md`「清单多的几格」第 2 条）：`miyu check` 报一条警告；
/// 有的不报。
#[tokio::test]
async fn a_page_without_an_entry_is_a_warning() {
    let home = Home::new();
    let program = Program::new();
    home.write(
        "home/alice/packages/relay.toml",
        &relay(&program.name(), "", "\n[page]\ndir = \"page\"\n"),
    );
    let mut client = Client::connect(core(&home, quick()));
    client.hello().await;
    let pages = |reply: &Value| -> Vec<Value> {
        reply["result"]["problems"]
            .as_array()
            .unwrap_or_else(|| panic!("{reply}"))
            .iter()
            .filter(|problem| problem["code"] == "page_missing")
            .cloned()
            .collect()
    };
    let reply = client.call("c1", "check", json!({})).await;
    assert_eq!(
        pages(&reply),
        [json!({
            "kind": "package",
            "file": "home/alice/packages/relay.toml",
            "code": "page_missing",
            "level": "warning",
            "message": "page 里没有 index.html",
        })],
        "{reply}"
    );
    home.write("home/alice/packages/relay/page/index.html", "<p>relay</p>");
    let reply = client.call("c2", "check", json!({})).await;
    assert!(pages(&reply).is_empty(), "{reply}");
}
