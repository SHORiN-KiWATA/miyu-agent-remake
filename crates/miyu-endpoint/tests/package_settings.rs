//! 包的配置项（施工 9-1 下，`docs/blueprint/packages.md`「配置项」）：真核心走一遍，核心照 `miyu-core` 起来时那样拼配置清单
//! （核心自己的几项、照清单 `settle` 拼的包的几项）。`config.schema` 里有它们，挂在「软件包」那一页、这个包那一组，名字和
//! 说明照连接的语言从清单来，隐藏的带标记；写进系统配置读得到最终值，写错 `check` 报；编号撞了核心自己的模块的，整份报
//! `settings_taken`、一项都不收。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_endpoint::config::{Config, Environment};
use miyu_session::testkit::Script;

use crate::support::*;

/// 一个带两项配置的界面包。
const CLOCK: &str = r#"[package]
kind = "ui"
protocol = [1, 1]
name = { en = "Clock", zh = "时钟" }

[settings.port]
type = "int"
min = 1
max = 65535
default = 8400
layers = ["system"]
name = { en = "Port", zh = "端口" }
description = { en = "Which port it listens on.", zh = "听哪个端口。" }

[settings.idle_seconds]
type = "int"
min = 1
default = 600
hidden = true
name = { en = "Idle seconds" }

[settings.zones]
type = "list"
element = "text"
max = 40
default = ["UTC"]
layers = ["system"]
name = { en = "Time zones", zh = "时区" }
"#;

/// 照 `miyu-core` 起来时那样造核心：清单读一次，照核心自己的几项 `settle`，拼进配置清单。
fn core(home: &Home) -> Arc<Core> {
    let core_items = [
        miyu_endpoint::settings::UiSettings::ITEMS,
        miyu_endpoint::settings::PersonaSettings::ITEMS,
        miyu_endpoint::settings::PresetSettings::ITEMS,
        miyu_endpoint::settings::PermissionSettings::ITEMS,
    ]
    .concat();
    let resources = miyu_store::resources::ResourceRoot::at(default_resources());
    let alice = miyu_kernel::id::AccountId::parse("alice").expect("账号合写法");
    let mut found = miyu_endpoint::packages::load(&resources, &home.root, &alice);
    let packaged = miyu_endpoint::packages::settle(&mut found, &core_items);
    let items = core_items.into_iter().chain(packaged).collect();
    let config = Config::load(&home.root, &alice, None, items, Environment::of(&[]));
    Arc::new(
        home.core_full(&Script::new([]), miyu_tool::Catalog::default(), None, TOKEN)
            .with_config(config)
            .with_packages(found),
    )
}

async fn connected(home: &Home) -> Client {
    let mut client = Client::connect(core(home));
    client.hello().await;
    client
}

fn item<'a>(schema: &'a Value, key: &str) -> &'a Value {
    schema["items"]
        .as_array()
        .expect("有项")
        .iter()
        .find(|item| item["key"] == key)
        .unwrap_or_else(|| panic!("没有 {key}：{schema}"))
}

#[tokio::test]
async fn package_settings_join_the_schema_on_the_packages_page() {
    let home = Home::new();
    home.write("home/alice/packages/clock/package.toml", CLOCK);
    let mut client = connected(&home).await;
    let schema = client.call("s1", "config.schema", json!({})).await["result"].clone();
    let port = item(&schema, "clock.port");
    assert_eq!(port["name"], "端口", "照连接的语言");
    assert_eq!(port["description"], "听哪个端口。");
    assert_eq!(port["type"], "int");
    assert_eq!(
        (port["min"].clone(), port["max"].clone()),
        (json!(1), json!(65535))
    );
    assert_eq!(port["default"], 8400);
    assert_eq!(port["layers"], json!(["system"]));
    assert_eq!(port["applies"], "head_start");
    assert_eq!(
        (port["page"].clone(), port["group"].clone()),
        (json!("packages"), json!("clock"))
    );
    assert_eq!(port["control"], "number");
    assert_eq!(port["package"], "clock", "归哪个包（施工 F-6 上）");
    assert!(
        item(&schema, "ui.language").get("package").is_none(),
        "核心自己的项不带 package"
    );
    assert!(port.get("hidden").is_none(), "露的不写这一格");
    let idle = item(&schema, "clock.idle_seconds");
    assert_eq!(idle["hidden"], true);
    assert_eq!(idle["name"], "Idle seconds", "没写中文的照英文");
    assert_eq!(idle["description"], "");
    let zones = item(&schema, "clock.zones");
    assert_eq!(
        (zones["type"].clone(), zones["element"].clone()),
        (json!("list"), json!("text")),
        "列表照核心自己的列表写（施工 9-1 补）"
    );
    assert_eq!(zones["control"], "list");
    assert_eq!(zones["default"], json!(["UTC"]));
    assert!(
        schema["pages"]
            .as_array()
            .expect("有")
            .contains(&json!({"id": "packages", "name": "软件包"}))
    );
    assert!(
        schema["groups"]
            .as_array()
            .expect("有")
            .contains(&json!({"id": "clock", "name": "时钟", "page": "packages"}))
    );
}

#[tokio::test]
async fn package_settings_have_final_values_and_wrong_ones_are_checked() {
    let home = Home::new();
    home.write("home/alice/packages/clock/package.toml", CLOCK);
    home.write("system/config.toml", "[clock]\nport = 9000\n");
    let mut client = connected(&home).await;
    let got = client
        .call(
            "g1",
            "config.get",
            json!({"keys": ["clock.port", "clock.idle_seconds", "clock.zones"]}),
        )
        .await;
    let values = got["result"]["items"].clone();
    assert_eq!(values["clock.port"]["value"], 9000, "{got}");
    assert_eq!(values["clock.idle_seconds"]["value"], 600, "{got}");
    assert_eq!(values["clock.zones"]["value"], json!(["UTC"]), "{got}");
    home.write(
        "system/config.toml",
        "[clock]\nzones = [\"UTC\", \"Asia/Tokyo\"]\n",
    );
    let mut client = connected(&home).await;
    let got = client
        .call("g2", "config.get", json!({"keys": ["clock.zones"]}))
        .await;
    assert_eq!(
        got["result"]["items"]["clock.zones"]["value"],
        json!(["UTC", "Asia/Tokyo"]),
        "{got}"
    );
    for wrong in ["port = 70000", "zones = [\"UTC\", 1]"] {
        home.write("system/config.toml", &format!("[clock]\n{wrong}\n"));
        let checked = client.call("c1", "check", json!({})).await;
        let problems = checked["result"]["problems"].as_array().expect("有");
        assert!(
            problems
                .iter()
                .any(|problem| problem["file"] == "system/config.toml"
                    && problem["kind"] == "config"),
            "{wrong}：{checked}"
        );
    }
    home.write("home/alice/settings.toml", "[clock]\nport = 9000\n");
    let checked = client
        .call(
            "c2",
            "check",
            json!({"file": home.root.path().join("home/alice/settings.toml").to_string_lossy()}),
        )
        .await;
    assert_eq!(
        checked["result"]["problems"][0]["code"], "wrong_layer",
        "只能写在系统配置：{checked}"
    );
}

#[tokio::test]
async fn a_package_named_like_a_core_module_declares_no_settings() {
    let home = Home::new();
    home.write(
        "home/alice/packages/ui/package.toml",
        &CLOCK.replace("Clock", "Ui"),
    );
    let mut client = connected(&home).await;
    let listed = client.call("p1", "package.list", json!({})).await;
    let ui = listed["result"]["packages"]
        .as_array()
        .expect("有项")
        .iter()
        .find(|package| package["package"] == "ui")
        .cloned()
        .expect("有");
    assert_eq!(ui["code"], "settings_taken", "{listed}");
    let schema = client.call("s1", "config.schema", json!({})).await;
    assert!(
        !schema["result"]["items"]
            .as_array()
            .expect("有")
            .iter()
            .any(|item| item["key"] == "ui.port"),
        "一项都不收"
    );
}

/// 平台接入的包（施工 F-6 上，`package-pages.md`：「接入」页去掉了）：它的配置项和别的包一样在「软件包」那一页、带 `package`。
/// 程序要在测试程序旁边，不然当没装（`package_switch.rs` 测）。
#[tokio::test]
async fn a_connection_package_has_its_settings_on_the_packages_page() {
    let home = Home::new();
    let program = crate::support::extensions::Program::new();
    let relay = format!(
        "[package]\nkind = \"process\"\nprotocol = [1, 1]\nname = {{ en = \"Relay\", zh = \"中转\" }}\n\n[command]\nname = \"relay\"\nprogram = \"{}\"\nabout = {{ en = \"Relay\" }}\n\n[process]\n\n[connection]\nplatform = \"relay\"\n\n[settings.port]\ntype = \"int\"\nlayers = [\"system\"]\nname = {{ en = \"Port\", zh = \"端口\" }}\n",
        program.name()
    );
    home.write("home/alice/packages/relay/package.toml", &relay);
    let mut client = connected(&home).await;
    let schema = client.call("s1", "config.schema", json!({})).await["result"].clone();
    let port = item(&schema, "relay.port");
    assert_eq!(
        (
            port["page"].clone(),
            port["group"].clone(),
            port["package"].clone()
        ),
        (json!("packages"), json!("relay"), json!("relay"))
    );
    let pages = schema["pages"].as_array().expect("有");
    assert!(
        !pages.iter().any(|page| page["id"] == "connections"),
        "{pages:?}"
    );
}
