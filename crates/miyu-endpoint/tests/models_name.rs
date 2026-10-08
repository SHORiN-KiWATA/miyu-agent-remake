//! 供应商的显示名（施工 8-21，`docs/blueprint/models.md`「对外的样子」`name`、「协议」`model.list`）：`model.list` 每一家
//! 多一格 `name`。写了的照写的，带文件、行、层；只有空白的当没写；没写的、对上了目录的照目录里那一家的名字；都没有的照
//! 编号；用不了的那一家也有。`key` 是完整的配置键名，头照抄它发 `config.set`。改了当场就照新的答。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_endpoint::config::{Config, Environment};
use miyu_models::catalog::{Catalog, CatalogSource, Loaded};
use miyu_models::matching::Vendors;
use miyu_models::profile::Profiles;
use miyu_models::settings::{ProviderSettings, UseSettings};
use miyu_session::testkit::Script;
use miyu_session::{ModelData, Observed};
use miyu_store::resources::ResourceRoot;
use miyu_tool::Catalog as ToolCatalog;

use crate::support::*;

/// 带上真目录裁出来的一份的模型资料：目录里 `deepseek` 那一家叫「DeepSeek」。
fn data() -> Arc<ModelData> {
    let profiles = Profiles::parse(&json!({
        "npm": {"@ai-sdk/openai-compatible": "openai-chat"},
        "providers": {"deepseek": {"driver": "openai-chat", "base_url": "https://api.deepseek.com"}}
    }))
    .expect("档案写法对");
    let vendors = Vendors::parse(&json!({"deepseek": ["deepseek"]})).expect("读得进");
    let data = ModelData::new(profiles, vendors, None);
    data.loaded(
        Some(Loaded {
            catalog: Catalog::parse(include_str!(
                "../../miyu-models/testdata/models-dev-trimmed.json"
            ))
            .expect("读得进")
            .catalog,
            source: CatalogSource::Snapshot,
            fetched: "2026-10-01T03:25:54.000Z".to_string(),
        }),
        Observed::default(),
    );
    Arc::new(data)
}

fn core(home: &Home) -> Arc<Core> {
    let items = [
        miyu_endpoint::settings::UiSettings::ITEMS,
        UseSettings::ITEMS,
        ProviderSettings::ITEMS,
    ]
    .concat();
    let config = Config::load(&home.root, &alice(), None, items, Environment::of(&[]));
    let core = Core::new(
        home.root.clone(),
        ResourceRoot::at(default_resources()),
        Arc::new(Script::new([])),
        ToolCatalog::default(),
        None,
        alice(),
        TOKEN.to_string(),
    );
    Arc::new(core.with_config(config).with_model_data(data()))
}

/// 每一家的 `name`，照编号。
fn names(reply: &Value) -> Vec<(String, Value)> {
    reply["result"]["providers"]
        .as_array()
        .unwrap_or_else(|| panic!("有供应商：{reply}"))
        .iter()
        .map(|provider| {
            (
                provider["id"].as_str().unwrap_or_default().to_string(),
                provider["name"].clone(),
            )
        })
        .collect()
}

#[tokio::test]
async fn a_written_name_then_the_catalog_then_the_id() {
    let home = Home::new();
    home.write(
        "system/config.toml",
        "[providers.deepseek]\nkeys = []\n\n\
         [providers.relay]\nname = \"  我的中转 \"\ndriver = \"openai-chat\"\nbase_url = \"https://relay.example.invalid/v1\"\n\n\
         [providers.plain]\ndriver = \"openai-chat\"\nbase_url = \"https://plain.example.invalid/v1\"\n\n\
         [providers.blank]\nname = \"   \"\ndriver = \"openai-chat\"\nbase_url = \"https://blank.example.invalid/v1\"\n\n\
         [providers.broken]\nname = \"坏了的那家\"\n",
    );
    let mut client = Client::connect(core(&home));
    client.hello().await;
    let reply = client.call("l", "model.list", json!({})).await;
    let key = |id: &str| format!("providers.{id}.name");
    assert_eq!(
        names(&reply),
        [
            (
                "blank".to_string(),
                json!({"value": "blank", "from": "id", "key": key("blank")})
            ),
            (
                "broken".to_string(),
                json!({"value": "坏了的那家", "from": "config", "file": "system/config.toml", "line": 19, "layer": "system", "key": key("broken")})
            ),
            (
                "deepseek".to_string(),
                json!({"value": "DeepSeek", "from": "catalog", "key": key("deepseek")})
            ),
            (
                "plain".to_string(),
                json!({"value": "plain", "from": "id", "key": key("plain")})
            ),
            (
                "relay".to_string(),
                json!({"value": "我的中转", "from": "config", "file": "system/config.toml", "line": 5, "layer": "system", "key": key("relay")})
            ),
        ],
        "{reply}"
    );
    assert!(
        reply["result"]["providers"][1]["problem"].is_string(),
        "用不了的那一家也有名字"
    );
}

#[tokio::test]
async fn a_name_set_now_shows_at_once_and_stays_in_bounds() {
    let home = Home::new();
    home.write("system/config.toml", "[providers.deepseek]\nkeys = []\n");
    let mut client = Client::connect(core(&home));
    client.hello().await;
    let set = client
        .call(
            "config-9f2c4e1a7b3d5f60-1",
            "config.set",
            json!({"layer": "personal", "changes": [{"key": "providers.deepseek.name", "input": "深度求索"}]}),
        )
        .await;
    assert_eq!(
        set["result"]["keys"]["providers.deepseek.name"]["applies"], "now",
        "{set}"
    );
    let reply = client.call("l", "model.list", json!({})).await;
    assert_eq!(
        names(&reply)[0].1["value"],
        "深度求索",
        "个人设置当场盖过目录：{reply}"
    );
    assert_eq!(names(&reply)[0].1["layer"], "personal");
    // 超过 64 个字符的照清单拒。
    let long = client
        .call(
            "config-9f2c4e1a7b3d5f60-2",
            "config.set",
            json!({"layer": "personal", "changes": [{"key": "providers.deepseek.name", "input": "名".repeat(65)}]}),
        )
        .await;
    assert!(long.get("error").is_some(), "{long}");
}
