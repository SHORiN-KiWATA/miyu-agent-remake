//! `venue.binding`（施工 O-31 前，`venues.md`「问对应表」）：核心拉起的、以系统账号连进来的扩展问一个平台身份在主人对应表里
//! 对着谁：对着管理员的回账号，没写的、对着不存在的账号的回 `null`；身份写错的（空的）、多写格的、一次问几个的 `bad_params`；本机的头（不是系统
//! 账号）回 `no_system_account`。只读：对应表照旧。

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_session::testkit::Script;
use miyu_tool::Catalog;

use crate::support::extensions::*;
use crate::support::venues::BINDINGS;
use crate::support::*;

/// 照对应表、读好的清单起来的核心，拉起开着的扩展。
fn served_core(home: &Home) -> Arc<Core> {
    home.write("system/config.toml", BINDINGS);
    let items = [
        miyu_endpoint::settings::UiSettings::ITEMS,
        miyu_endpoint::settings::PersonaSettings::ITEMS,
        miyu_endpoint::settings::PresetSettings::ITEMS,
        miyu_endpoint::settings::PermissionSettings::ITEMS,
        miyu_endpoint::settings::EXTERNAL_BINDINGS,
    ]
    .concat();
    let resources = miyu_store::resources::ResourceRoot::at(default_resources());
    let found = miyu_endpoint::packages::load(&resources, &home.root, &alice());
    let config = miyu_endpoint::config::Config::load(
        &home.root,
        &alice(),
        None,
        items,
        miyu_endpoint::config::Environment::of(&[]),
    );
    let core = Arc::new(
        home.core_full(&Script::new([]), Catalog::default(), None, TOKEN)
            .with_extension_timing(quick())
            .with_config(config)
            .with_packages(found),
    );
    core.start_extensions();
    core
}

/// 等到记下的回应有 `n` 行，交回它们。
async fn replies(path: &Path, n: usize) -> Vec<Value> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let got: Vec<Value> = read(path)
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();
        if got.len() >= n {
            return got;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "等不到 {n} 行：{got:?}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn a_bridge_asks_who_a_platform_id_is_bound_to() {
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "bot");
    install_serving(
        &home,
        "bot",
        &program.name(),
        &steps(&[
            &step,
            "hello",
            r#"ask:venue.binding:{"id":"qq:10001"}"#,
            r#"ask:venue.binding:{"id":"qq:20001"}"#,
            r#"ask:venue.binding:{"id":"qq:10003"}"#,
            r#"ask:venue.binding:{"id":""}"#,
            r#"ask:venue.binding:{"id":"qq:10001","more":1}"#,
            r#"ask:venue.binding:{"ids":["qq:10001"]}"#,
            "wait",
        ]),
        "always",
        true,
    );
    let core = served_core(&home);
    let got = replies(&path, 7).await;
    assert_eq!(got[0]["result"]["account"], "bot", "这个连接是系统账号");
    assert_eq!(got[1]["result"], json!({"account": "alice"}), "{got:?}");
    assert_eq!(got[2]["result"], json!({"account": null}), "没写的");
    assert_eq!(
        got[3]["result"],
        json!({"account": null}),
        "对着不存在的账号的当没写"
    );
    for refused in &got[4..7] {
        assert_eq!(
            refused["error"]["data"]["reason"], "bad_params",
            "{refused}"
        );
    }

    // 本机的头不是系统账号。
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let refused = client
        .call("b1", "venue.binding", json!({"id": "qq:10001"}))
        .await;
    assert_eq!(reason(&refused), Some("no_system_account"), "{refused}");
    assert_eq!(
        std::fs::read_to_string(home.root.system().join("config.toml")).expect("读得出"),
        BINDINGS,
        "只读"
    );
}
