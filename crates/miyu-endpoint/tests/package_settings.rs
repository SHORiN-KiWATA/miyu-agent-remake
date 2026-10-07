//! 包的配置项（施工 9-1 下，`docs/blueprint/packages.md`「配置项」）：真核心走一遍，核心照 `miyu-core` 起来时那样拼配置清单
//! （核心自己的几项、照清单 `settle` 拼的包的几项）。`config.schema` 里有它们，挂在「软件包」那一页、这个包那一组，名字和
//! 说明照连接的语言从清单来，隐藏的带标记；写进系统配置读得到最终值，写错 `check` 报；编号撞了核心自己的模块的，整份报
//! `settings_taken`、一项都不收。

mod support;

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_endpoint::config::{Config, Environment};
use miyu_session::testkit::Script;

use support::*;

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
"#;

/// 照 `miyu-core` 起来时那样造核心：清单读一次，照核心自己的几项 `settle`，拼进配置清单。
fn core(home: &Home) -> Arc<Core> {
    let core_items = [
        miyu_endpoint::settings::UiSettings::ITEMS,
        miyu_endpoint::settings::PersonaSettings::ITEMS,
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
    home.write("home/alice/packages/clock.toml", CLOCK);
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
    assert!(port.get("hidden").is_none(), "露的不写这一格");
    let idle = item(&schema, "clock.idle_seconds");
    assert_eq!(idle["hidden"], true);
    assert_eq!(idle["name"], "Idle seconds", "没写中文的照英文");
    assert_eq!(idle["description"], "");
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
    home.write("home/alice/packages/clock.toml", CLOCK);
    home.write("system/config.toml", "[clock]\nport = 9000\n");
    let mut client = connected(&home).await;
    let got = client
        .call(
            "g1",
            "config.get",
            json!({"keys": ["clock.port", "clock.idle_seconds"]}),
        )
        .await;
    let values = got["result"]["items"].clone();
    assert_eq!(values["clock.port"]["value"], 9000, "{got}");
    assert_eq!(values["clock.idle_seconds"]["value"], 600, "{got}");
    home.write("system/config.toml", "[clock]\nport = 70000\n");
    let checked = client.call("c1", "check", json!({})).await;
    let problems = checked["result"]["problems"].as_array().expect("有");
    assert!(
        problems
            .iter()
            .any(|problem| problem["file"] == "system/config.toml" && problem["kind"] == "config"),
        "{checked}"
    );
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
    home.write("home/alice/packages/ui.toml", &CLOCK.replace("Clock", "Ui"));
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
