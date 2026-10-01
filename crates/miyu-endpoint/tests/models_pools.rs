//! 用途、挡位、池在协议上的样子（施工 8-8，`docs/blueprint/models.md`「协议」）：`model.list` 多 `pools`、`tiers`，`uses` 多
//! `vision`，用途挡位池里点名的模型也列；`session.create` 的 `model` 照这时的配置解析好记进 `session.created`，解析不出的
//! `unknown_model`、什么都不造；派子代理时子会话照挡位、父会话记下的；引用的供应商、池没配的，配置的问题里报
//! `bad_reference`、算进 `config_errors`。

mod support;

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_endpoint::config::{Config, Environment};
use miyu_kernel::event::Body;
use miyu_models::matching::Vendors;
use miyu_models::profile::Profiles;
use miyu_models::settings::{
    CatalogSettings, ModelSettings, PoolSettings, PriceSettings, ProviderSettings, TierSettings,
    UseSettings,
};
use miyu_session::testkit::{Play, Script};
use miyu_session::{ModelData, Observed};
use miyu_store::resources::ResourceRoot;
use miyu_tool::Catalog as ToolCatalog;

use support::*;

/// 两家 `a`、`b`（`b` 按次计费），主对话 `a/m`、看图 `b/v`，`lite` 是池 `@free`、`flagship` 是 `b/big`；池 `free` 钉住（成员
/// 有一个认不出），池 `fast` 不写分法（成员全是按次计费的：轮换），池 `empty` 一个成员都认不出。
const CONFIG: &str = "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"https://a.invalid\"\n\n\
[providers.b]\ndriver = \"openai-chat\"\nbase_url = \"https://b.invalid\"\ncache = \"per_request\"\n\n\
[models]\nchat = \"a/m\"\nvision = \"b/v\"\n\n[models.tiers]\nlite = \"@free\"\nflagship = \"b/big\"\n\n\
[pools.free]\nmodels = [\"a/x\", \"gone/y\", \"b/z\"]\nstrategy = \"pin\"\n\n[pools.fast]\nmodels = [\"b/z\"]\n\n\
[pools.empty]\nmodels = [\"gone/y\"]\n";

/// 一份核心：清单带上模型这一块，配置照磁盘上现在的几份读，没有目录。请求模型照 `script`，工具照 `tools`。
fn core_with(home: &Home, script: Script, tools: ToolCatalog) -> Arc<Core> {
    let items = [
        miyu_endpoint::settings::UiSettings::ITEMS,
        UseSettings::ITEMS,
        TierSettings::ITEMS,
        PoolSettings::ITEMS,
        ProviderSettings::ITEMS,
        ModelSettings::ITEMS,
        PriceSettings::ITEMS,
        CatalogSettings::ITEMS,
    ]
    .concat();
    let config = Config::load(&home.root, &alice(), None, items, Environment::of(&[]));
    let data = ModelData::new(
        Profiles::parse(&json!({})).expect("档案写法对"),
        Vendors::default(),
        None,
    );
    data.loaded(None, Observed::default());
    let core = Core::new(
        home.root.clone(),
        ResourceRoot::at(default_resources()),
        Arc::new(script),
        tools,
        None,
        alice(),
        TOKEN.to_string(),
    );
    Arc::new(core.with_config(config).with_model_data(Arc::new(data)))
}

/// 同 [`core_with`]，用不着模型、没有工具。
fn core(home: &Home) -> Arc<Core> {
    core_with(home, Script::new([]), ToolCatalog::default())
}

/// 连上、握手。
async fn connect(home: &Home) -> (Client, Value) {
    let mut client = Client::connect(core(home));
    let hello = client.hello().await;
    (client, hello)
}

/// 这一家列出来的模型名。
fn models(list: &Value, provider: &str) -> Vec<String> {
    let providers = list["providers"].as_array().expect("有供应商");
    let found = providers
        .iter()
        .find(|entry| entry["id"] == provider)
        .unwrap_or_else(|| panic!("没有 {provider}"));
    found["models"]
        .as_array()
        .expect("有模型")
        .iter()
        .filter_map(|model| model["model"].as_str().map(str::to_string))
        .collect()
}

#[tokio::test]
async fn model_list_has_pools_tiers_and_both_uses() {
    let home = Home::new();
    home.write("system/config.toml", CONFIG);
    let (mut client, _) = connect(&home).await;
    let reply = client.call("l1", "model.list", json!({})).await;
    let result = &reply["result"];
    assert_eq!(
        result["pools"],
        json!([
            {"name": "empty", "strategy": "pin", "models": ["gone/y"]},
            {"name": "fast", "strategy": "rotate", "models": ["b/z"]},
            {"name": "free", "strategy": "pin", "models": ["a/x", "gone/y", "b/z"]},
        ]),
        "照名字排，成员照写的原样，分法没写的照成员定"
    );
    assert_eq!(
        result["tiers"],
        json!({"lite": "@free", "cheap": null, "standard": null, "flagship": "b/big"})
    );
    assert_eq!(result["uses"], json!({"chat": "a/m", "vision": "b/v"}));
    assert_eq!(models(result, "a"), ["m", "x"], "用途、池里点名的");
    assert_eq!(
        models(result, "b"),
        ["big", "v", "z"],
        "挡位、用途、池里点名的"
    );
    // 什么都没配的：池是空的，挡位、用途都是 null。
    let bare = Home::new();
    let (mut client, _) = connect(&bare).await;
    let reply = client.call("l2", "model.list", json!({})).await;
    assert_eq!(reply["result"]["pools"], json!([]));
    assert_eq!(
        reply["result"]["tiers"],
        json!({"lite": null, "cheap": null, "standard": null, "flagship": null})
    );
    assert_eq!(
        reply["result"]["uses"],
        json!({"chat": null, "vision": null})
    );
}

/// 造会话 `id`，`model` 照 `params` 写，交回回应。
async fn create(client: &mut Client, id: &str, model: Value) -> Value {
    client
        .call(id, "session.create", json!({"cwd": "/tmp", "model": model}))
        .await
}

/// 造出来的会话的 `session.created` 记着的模型。
fn recorded(home: &Home, reply: &Value) -> Option<String> {
    let session = reply["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("造出了会话：{reply}"));
    match &home.log(session)[0].body {
        Body::SessionCreated(created) => created.model.clone(),
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn session_create_records_the_resolved_model_or_refuses_it() {
    let home = Home::new();
    home.write("system/config.toml", CONFIG);
    let (mut client, _) = connect(&home).await;
    for (n, (model, wanted)) in [
        (json!("lite"), "@free"),
        (json!("flagship"), "b/big"),
        (json!("cheap"), "a/m"),
        (json!("b/anything"), "b/anything"),
        (json!("@fast"), "@fast"),
        (Value::Null, "a/m"),
    ]
    .into_iter()
    .enumerate()
    {
        let reply = create(&mut client, &format!("ok-{n}"), model.clone()).await;
        assert_eq!(recorded(&home, &reply).as_deref(), Some(wanted), "{model}");
    }
    let made = home.root.sessions(&alice()).expect("读得了").len();
    for (n, model) in ["c/m", "@nope", "@empty", "nope", "Lite"]
        .into_iter()
        .enumerate()
    {
        let reply = create(&mut client, &format!("no-{n}"), json!(model)).await;
        assert_eq!(reason(&reply), Some("unknown_model"), "{model}：{reply}");
    }
    let reply = create(&mut client, "bad", json!(3)).await;
    assert_eq!(reason(&reply), Some("bad_params"));
    assert_eq!(
        home.root.sessions(&alice()).expect("读得了").len(),
        made,
        "解析不出的什么都不造"
    );
    // 挡位没配、`models.chat` 也没配：解析不出。
    let bare = Home::new();
    bare.write("system/config.toml", "[providers.a]\nkeys = []\n");
    let (mut client, _) = connect(&bare).await;
    let reply = create(&mut client, "tier", json!("lite")).await;
    assert_eq!(reason(&reply), Some("unknown_model"));
    let reply = create(&mut client, "plain", Value::Null).await;
    assert_eq!(recorded(&bare, &reply), None, "chat 也没配的不写");
}

#[tokio::test]
async fn a_reference_to_what_is_not_configured_is_a_bad_reference() {
    let home = Home::new();
    home.write(
        "system/config.toml",
        "[ui]\nlanguage = \"en\"\n\n[providers.a]\nkeys = []\n\n[models]\nchat = \"c/m\"\n\n[models.tiers]\nlite = \"@nope\"\n\n[pools.p]\nmodels = [\"a/x\", \"d/y\"]\n",
    );
    let (mut client, hello) = connect(&home).await;
    assert_eq!(hello["result"]["config_errors"], 3, "{hello}");
    let reply = client.call("g", "config.get", json!({})).await;
    let problems: Vec<(String, String)> = reply["result"]["problems"]
        .as_array()
        .expect("有问题")
        .iter()
        .map(|problem| {
            (
                problem["code"].as_str().unwrap_or_default().to_string(),
                problem["key"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect();
    assert_eq!(
        problems,
        [
            ("bad_reference".to_string(), "models.chat".to_string()),
            ("bad_reference".to_string(), "models.tiers.lite".to_string()),
            ("bad_reference".to_string(), "pools.p.models".to_string()),
        ]
    );
    let first = &reply["result"]["problems"][0];
    assert_eq!(first["level"], "error");
    assert_eq!(
        first["message"],
        "models.chat points at the provider c, which is not configured."
    );
    // 查一段还没生效的字：字里新配的供应商算上，不报。
    let checked = client
        .call(
            "c",
            "config.check",
            json!({"layer": "system", "text": "[providers.c]\nkeys = []\n\n[models]\nchat = \"c/m\"\n"}),
        )
        .await;
    assert_eq!(checked["result"]["problems"], json!([]), "{checked}");
}

/// 真核心派子代理（施工 8-8）：写了挡位的照这时的配置解析，没写的抄父会话记下的，记进子会话的 `session.created`。
#[tokio::test]
async fn a_child_session_records_the_tier_or_its_parent_model() {
    let home = Home::new();
    home.write("system/config.toml", CONFIG);
    let tools = ToolCatalog::new(miyu_basesystem::tools(&default_resources()).unwrap()).unwrap();
    let tiered = json!({"description": "轻量", "prompt": "Task.", "tier": "lite"}).to_string();
    let plain = json!({"description": "跟父会话", "prompt": "Task."}).to_string();
    let script = Script::new([
        Play::calls(&[("subagent", &tiered), ("subagent", &plain)]),
        Play::Says("好。"),
        Play::Says("好。"),
        Play::Says("好。"),
        Play::Says("好。"),
        Play::Says("好。"),
    ]);
    let mut client = Client::connect(core_with(&home, script, tools));
    client.hello().await;
    let work = home.work.to_string_lossy().into_owned();
    let reply = client
        .call(
            "c1",
            "session.create",
            json!({"cwd": work, "model": "flagship"}),
        )
        .await;
    assert_eq!(recorded(&home, &reply).as_deref(), Some("b/big"));
    let parent = reply["result"]["session"]
        .as_str()
        .expect("造出了")
        .to_string();
    client.say("c2", &parent, "派两个").await;
    home.until_turns(&parent, 1).await;
    let mut children: Vec<(String, Option<String>)> = home
        .log(&parent)
        .iter()
        .filter_map(|event| match &event.body {
            Body::ToolResult(result) => Some(result.effects.clone()),
            _ => None,
        })
        .flatten()
        .filter_map(|effect| match effect {
            miyu_kernel::event::Effect::JobStarted(started) => {
                let child = started.session?.to_string();
                let model = match &home.log(&child)[0].body {
                    Body::SessionCreated(created) => created.model.clone(),
                    _ => None,
                };
                Some((started.title, model))
            }
            _ => None,
        })
        .collect();
    children.sort();
    assert_eq!(
        children,
        [
            ("跟父会话".to_string(), Some("b/big".to_string())),
            ("轻量".to_string(), Some("@free".to_string())),
        ]
    );
}
