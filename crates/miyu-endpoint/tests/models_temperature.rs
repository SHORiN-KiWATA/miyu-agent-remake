//! 温度在协议上的样子（施工 8-22，`docs/blueprint/models.md`「协议」「怎么走」第十四条）：`model.list` 的 `facts` 多
//! `temperature`（配置的默认，带完整的配置键名 `key`）和 `takes_temperature`（目录说的能不能调）；配置里写的这个模型
//! 用不了的（目录说不收的、超过 Anthropic 的上限 1 的）报 `unusable_temperature`、算进 `config_errors`，`config.check`
//! 照新的字查；`config.set` 写温度下一轮生效，超出 0 到 2 的照清单拒。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_endpoint::config::{Config, Environment};
use miyu_models::catalog::{Catalog, CatalogSource, Loaded};
use miyu_models::matching::Vendors;
use miyu_models::profile::Profiles;
use miyu_models::settings::{ModelSettings, PriceSettings, ProviderSettings, UseSettings};
use miyu_session::testkit::Script;
use miyu_session::{ModelData, Observed};
use miyu_store::resources::ResourceRoot;
use miyu_tool::Catalog as ToolCatalog;

use crate::support::*;

/// 三家照档案认得：`deepseek` 走 `openai-chat`、`anthropic` 走 `anthropic`、`openai` 走 `openai-responses`。裁出来的目录里
/// `deepseek-flash`、`claude-sonnet-4-5` 能调，`gpt-5` 不能调。
fn data() -> Arc<ModelData> {
    let profiles = Profiles::parse(&json!({
        "npm": {"@ai-sdk/openai-compatible": "openai-chat", "@ai-sdk/anthropic": "anthropic", "@ai-sdk/openai": "openai-responses"},
        "providers": {
            "deepseek": {"driver": "openai-chat", "base_url": "https://api.deepseek.com"},
            "anthropic": {"driver": "anthropic", "base_url": "https://api.anthropic.com"},
            "openai": {"driver": "openai-responses", "base_url": "https://api.openai.com/v1"}
        }
    }))
    .expect("档案写法对");
    let vendors = Vendors::parse(&json!({
        "deepseek": ["deepseek"], "claude": ["anthropic"], "gpt": ["openai"]
    }))
    .expect("读得进");
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
        ModelSettings::ITEMS,
        PriceSettings::ITEMS,
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

/// 三家各配一个模型：`deepseek-flash` 0.7，`gpt-5` 0.2（不收），`claude-sonnet-4-5` 1.5（超过 1）。
const CONFIG: &str = "[ui]\nlanguage = \"en\"\n\n\
[providers.deepseek]\nlocal = false\n\n[providers.deepseek.models.\"deepseek-flash\"]\ntemperature = 0.7\n\n\
[providers.openai]\nlocal = false\n\n[providers.openai.models.\"gpt-5\"]\ntemperature = 0.2\n\n\
[providers.anthropic]\nlocal = false\n\n[providers.anthropic.models.\"claude-sonnet-4-5\"]\ntemperature = 1.5\n";

/// `model.list` 里 `provider` 这一家的 `model` 的 `facts`。
fn facts(reply: &Value, provider: &str, model: &str) -> Value {
    reply["result"]["providers"]
        .as_array()
        .unwrap_or_else(|| panic!("有供应商：{reply}"))
        .iter()
        .find(|entry| entry["id"] == provider)
        .and_then(|entry| {
            entry["models"]
                .as_array()?
                .iter()
                .find(|entry| entry["model"] == model)
        })
        .map(|entry| entry["facts"].clone())
        .unwrap_or_else(|| panic!("没有 {provider}/{model}：{reply}"))
}

/// `config.get` 的问题：原因码、键、话。
fn problems(reply: &Value) -> Vec<(String, String, String)> {
    reply["result"]["problems"]
        .as_array()
        .unwrap_or_else(|| panic!("有 problems：{reply}"))
        .iter()
        .map(|problem| {
            let text = |name: &str| problem[name].as_str().unwrap_or_default().to_string();
            (text("code"), text("key"), text("message"))
        })
        .collect()
}

#[tokio::test]
async fn model_list_shows_the_default_temperature_its_key_and_whether_the_model_takes_one() {
    let home = Home::new();
    home.write("system/config.toml", CONFIG);
    let mut client = Client::connect(core(&home));
    client.hello().await;
    let reply = client.call("l", "model.list", json!({})).await;
    let line = CONFIG
        .lines()
        .position(|line| line == "temperature = 0.7")
        .expect("写了")
        + 1;
    let deepseek = facts(&reply, "deepseek", "deepseek-flash");
    assert_eq!(
        deepseek["temperature"],
        json!({
            "value": 0.7, "from": "config", "file": "system/config.toml", "line": line,
            "layer": "system", "key": "providers.deepseek.models.deepseek-flash.temperature",
        })
    );
    assert_eq!(
        deepseek["takes_temperature"],
        json!({"value": true, "from": "catalog", "entry": "deepseek/deepseek-flash", "layer": 2, "fetched": "2026-10-01T03:25:54.000Z"})
    );
    let gpt = facts(&reply, "openai", "gpt-5");
    assert_eq!(gpt["takes_temperature"]["value"], false);
    assert_eq!(
        gpt["temperature"],
        json!({"value": null, "from": "default", "key": "providers.openai.models.gpt-5.temperature"}),
        "不收的照没写，key 照样给"
    );
    let claude = facts(&reply, "anthropic", "claude-sonnet-4-5");
    assert_eq!(claude["temperature"]["value"], Value::Null, "超过 1 照没写");
    assert_eq!(claude["takes_temperature"]["value"], true);
}

#[tokio::test]
async fn a_temperature_the_model_cannot_use_is_reported() {
    let home = Home::new();
    home.write("system/config.toml", CONFIG);
    let mut client = Client::connect(core(&home));
    let hello = client.hello().await;
    assert_eq!(hello["result"]["config_errors"], 2, "{hello}");
    let reply = client.call("g", "config.get", json!({})).await;
    assert_eq!(
        problems(&reply),
        [
            (
                "unusable_temperature".to_string(),
                "providers.anthropic.models.claude-sonnet-4-5.temperature".to_string(),
                "providers.anthropic.models.claude-sonnet-4-5.temperature is 1.5, above this model's limit of 1. Requests go out as if it were not set.".to_string(),
            ),
            (
                "unusable_temperature".to_string(),
                "providers.openai.models.gpt-5.temperature".to_string(),
                "providers.openai.models.gpt-5.temperature: this model does not take a temperature. Requests go out as if it were not set.".to_string(),
            ),
        ],
        "{reply}"
    );
    assert!(
        reply["result"]["problems"]
            .as_array()
            .is_some_and(|problems| problems.iter().all(|problem| problem["level"] == "error")),
        "{reply}"
    );
    // 查一段还没生效的字：照字里新写的查，改对了的不报。
    let text = CONFIG
        .replace("temperature = 1.5", "temperature = 1")
        .replace("temperature = 0.2\n", "");
    let checked = client
        .call(
            "c",
            "config.check",
            json!({"layer": "system", "text": text}),
        )
        .await;
    assert_eq!(checked["result"]["problems"], json!([]), "{checked}");
}

#[tokio::test]
async fn a_temperature_set_applies_next_turn_and_stays_in_bounds() {
    let home = Home::new();
    home.write(
        "system/config.toml",
        "[providers.deepseek]\nlocal = false\n",
    );
    let mut client = Client::connect(core(&home));
    client.hello().await;
    let key = "providers.deepseek.models.deepseek-flash.temperature";
    let set = client
        .call(
            "config-9f2c4e1a7b3d5f60-1",
            "config.set",
            json!({"layer": "personal", "changes": [{"key": key, "value": 0.4}]}),
        )
        .await;
    assert_eq!(set["result"]["keys"][key]["applies"], "next_turn", "{set}");
    let reply = client.call("l", "model.list", json!({})).await;
    let deepseek = facts(&reply, "deepseek", "deepseek-flash");
    assert_eq!(deepseek["temperature"]["value"], 0.4, "{reply}");
    assert_eq!(deepseek["temperature"]["layer"], "personal");
    for (n, value) in [json!(2.5), json!(-0.1), json!("hot")]
        .into_iter()
        .enumerate()
    {
        let refused = client
            .call(
                &format!("config-9f2c4e1a7b3d5f60-{}", n + 2),
                "config.set",
                json!({"layer": "personal", "changes": [{"key": key, "value": value}]}),
            )
            .await;
        assert!(refused.get("error").is_some(), "{value}：{refused}");
    }
}
