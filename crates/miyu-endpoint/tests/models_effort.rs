//! 思考强度在协议上的样子（施工 8-18；8-18（补）去掉会话那一层，`docs/blueprint/models.md`「协议」「怎么走」第十一条）：
//! `session.configure` 不再收 `effort`（写了的 `bad_params`），`model` 变回必写（没写的也 `bad_params`）；`subscribe` 的
//! `model` 多 `effort`，`from` 是配置的哪一层；`model.list` 的 `facts` 多 `effort`，带着完整的配置键名 `key`；配置里写的
//! 不在档位里的报 `unknown_effort`、算进 `config_errors`。
//!
//! 没有目录：档位全照手写的 `reasoning`。

mod support;

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_endpoint::config::{Config, Environment};
use miyu_models::matching::Vendors;
use miyu_models::profile::Profiles;
use miyu_models::settings::{
    CatalogSettings, ModelSettings, PoolSettings, PriceSettings, ProviderSettings, UseSettings,
};
use miyu_session::testkit::Script;
use miyu_session::{ModelData, Models, Observed, Routes};
use miyu_store::resources::ResourceRoot;
use miyu_tool::Catalog as ToolCatalog;

use support::*;

/// 两家 `a`、`b`。`a/m` 有 `off`、`low`、`high`，默认 `low`；`a/x` 只有 `high`，默认写成了它没有的 `max`；`b/n` 没写几档。
/// 主对话 `a/m`，池 `p` 轮换。
const CONFIG: &str = "[ui]\nlanguage = \"en\"\n\n\
[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"https://a.invalid\"\n\n\
[providers.a.models.m]\nreasoning = [\"off\", \"low\", \"high\"]\neffort = \"low\"\n\n\
[providers.a.models.x]\nreasoning = [\"high\"]\neffort = \"max\"\n\n\
[providers.b]\ndriver = \"openai-chat\"\nbase_url = \"https://b.invalid\"\n\n\
[models]\nchat = \"a/m\"\n\n\
[pools.p]\nmodels = [\"a/m\", \"b/n\"]\nstrategy = \"rotate\"\n";

/// 没有目录、读完了的模型资料。
fn no_catalog() -> Arc<ModelData> {
    let data = ModelData::new(
        Profiles::parse(&json!({})).expect("档案写法对"),
        Vendors::default(),
        None,
    );
    data.loaded(None, Observed::default());
    Arc::new(data)
}

/// 一份核心：清单带上模型这一块，配置照磁盘上现在的几份读；`routed` 的请求模型的端口是真的路由（不发请求）。
fn core(home: &Home, routed: bool) -> Arc<Core> {
    let data = no_catalog();
    let models: Arc<dyn Models> = match routed {
        true => Arc::new(Routes {
            client: miyu_http::client(miyu_http::Proxy::Off).expect("造得出客户端"),
            direct: miyu_http::client(miyu_http::Proxy::Off).expect("造得出客户端"),
            data: Arc::clone(&data),
            idle: std::time::Duration::from_secs(5),
        }),
        false => Arc::new(Script::new([])),
    };
    let items = [
        miyu_endpoint::settings::UiSettings::ITEMS,
        UseSettings::ITEMS,
        PoolSettings::ITEMS,
        ProviderSettings::ITEMS,
        ModelSettings::ITEMS,
        PriceSettings::ITEMS,
        CatalogSettings::ITEMS,
    ]
    .concat();
    let config = Config::load(&home.root, &alice(), None, items, Environment::of(&[]));
    let core = Core::new(
        home.root.clone(),
        ResourceRoot::at(default_resources()),
        models,
        ToolCatalog::default(),
        None,
        alice(),
        TOKEN.to_string(),
    );
    Arc::new(core.with_config(config).with_model_data(data))
}

/// 照 `params` 发一条 `session.configure`：交回回应之前读到的推送，和回应。
async fn configuring(client: &mut Client, id: &str, params: Value) -> (Vec<Value>, Value) {
    let request =
        json!({"jsonrpc": "2.0", "id": id, "method": "session.configure", "params": params});
    client.line(&request.to_string()).await;
    client.until_reply(id).await
}

/// `session.configure` 不再收 `effort`：写了的回 `bad_params`，不找会话、什么都不记（施工 8-18（补），「怎么走」原话
/// 「思考强度改在配置里」）。
#[tokio::test]
async fn session_configure_refuses_an_effort_parameter() {
    let home = Home::new();
    home.write("system/config.toml", CONFIG);
    let mut client = Client::connect(core(&home, false));
    client.hello().await;
    let session = client.create("c1", "/tmp").await;
    for (n, params) in [
        json!({"session": session, "model": "a/m", "effort": {"model": "a/m", "level": "high"}}),
        json!({"session": session, "effort": {"model": "a/m", "level": "high"}}),
    ]
    .into_iter()
    .enumerate()
    {
        let (_, reply) = configuring(&mut client, &format!("bad-{n}"), params.clone()).await;
        assert_eq!(reason(&reply), Some("bad_params"), "{params}：{reply}");
    }
}

/// `model` 变回必写（施工 8-18 把它改成了「`model`、`effort` 至少一个」，8-18（补）改回去）：没写的、空字的 `bad_params`，
/// 不找会话。
#[tokio::test]
async fn session_configure_requires_a_model() {
    let home = Home::new();
    home.write("system/config.toml", CONFIG);
    let mut client = Client::connect(core(&home, false));
    client.hello().await;
    let session = client.create("c1", "/tmp").await;
    for (n, params) in [
        json!({"session": session}),
        json!({"session": session, "model": null}),
        json!({"session": session, "model": ""}),
    ]
    .into_iter()
    .enumerate()
    {
        let (_, reply) = configuring(&mut client, &format!("bad-{n}"), params.clone()).await;
        assert_eq!(reason(&reply), Some("bad_params"), "{params}：{reply}");
    }
    // 写对了照旧能换：确认上面几条真的什么都没记。
    let (_, reply) = configuring(
        &mut client,
        "ok",
        json!({"session": session, "model": "@p"}),
    )
    .await;
    assert_eq!(reply["result"], json!({}), "{reply}");
}

#[tokio::test]
async fn subscribe_says_which_config_layer_the_next_effort_comes_from() {
    let home = Home::new();
    home.write("system/config.toml", CONFIG);
    let mut client = Client::connect(core(&home, true));
    client.hello().await;
    for (n, model, wanted) in [
        (
            1,
            "a/m",
            json!({"ref": "a/m", "endpoint": "a", "model": "m", "effort": {"level": "low", "from": "system"}}),
        ),
        (
            2,
            "b/n",
            json!({"ref": "b/n", "endpoint": "b", "model": "n"}),
        ),
        (3, "@p", json!({"ref": "@p"})),
    ] {
        let reply = client
            .call(
                &format!("c{n}"),
                "session.create",
                json!({"cwd": "/tmp", "model": model}),
            )
            .await;
        let session = reply["result"]["session"].as_str().expect("造出了会话");
        let reply = client.subscribe(&format!("s{n}"), session).await;
        assert_eq!(reply["result"]["model"], wanted, "{reply}");
    }
}

#[tokio::test]
async fn model_list_shows_the_default_effort_and_its_full_config_key() {
    let home = Home::new();
    home.write("system/config.toml", CONFIG);
    let mut client = Client::connect(core(&home, false));
    client.hello().await;
    let reply = client.call("l", "model.list", json!({})).await;
    let models = reply["result"]["providers"][0]["models"]
        .as_array()
        .expect("有模型");
    let facts = |name: &str| {
        models
            .iter()
            .find(|model| model["model"] == name)
            .map(|model| model["facts"].clone())
            .unwrap_or_else(|| panic!("没有 {name}"))
    };
    let line = CONFIG
        .lines()
        .position(|line| line == "effort = \"low\"")
        .expect("写了")
        + 1;
    assert_eq!(
        facts("m")["effort"],
        json!({
            "value": "low", "from": "config", "file": "system/config.toml", "line": line,
            "key": "providers.a.models.m.effort",
        })
    );
    assert_eq!(
        facts("x")["effort"],
        json!({"value": null, "from": "default", "key": "providers.a.models.x.effort"}),
        "写的不在档位里：照没写，key 照样给"
    );
    assert_eq!(
        facts("m")["reasoning"]["value"],
        json!(["off", "low", "high"])
    );
}

/// 模型名带点的：引号照 TOML 的写法补上（施工 8-18（补），头照抄这个键发 `config.set`）。
#[tokio::test]
async fn the_key_quotes_a_model_name_with_dots() {
    let home = Home::new();
    let config = "[providers.dev]\ndriver = \"openai-chat\"\nbase_url = \"https://dev.invalid\"\n\n\
        [providers.dev.models.\"deepseek-v4.1-flash\"]\nreasoning = [\"low\", \"high\"]\n";
    home.write("system/config.toml", config);
    let mut client = Client::connect(core(&home, false));
    client.hello().await;
    let reply = client.call("l", "model.list", json!({})).await;
    let models = reply["result"]["providers"][0]["models"]
        .as_array()
        .expect("有模型");
    let entry = models
        .iter()
        .find(|model| model["model"] == "deepseek-v4.1-flash")
        .expect("有这个模型");
    assert_eq!(
        entry["facts"]["effort"]["key"],
        json!("providers.dev.models.\"deepseek-v4.1-flash\".effort")
    );
}

#[tokio::test]
async fn a_default_effort_the_model_does_not_have_is_reported() {
    let home = Home::new();
    home.write("system/config.toml", CONFIG);
    let mut client = Client::connect(core(&home, false));
    let hello = client.hello().await;
    assert_eq!(hello["result"]["config_errors"], 1, "{hello}");
    let reply = client.call("g", "config.get", json!({})).await;
    let problems = reply["result"]["problems"].as_array().expect("有问题");
    assert_eq!(problems.len(), 1, "{reply}");
    let problem = &problems[0];
    assert_eq!(
        (&problem["code"], &problem["level"], &problem["key"]),
        (
            &json!("unknown_effort"),
            &json!("error"),
            &json!("providers.a.models.x.effort")
        )
    );
    assert_eq!(
        problem["message"],
        "providers.a.models.x.effort is max, which is not one of this model's levels now. Requests go out as if it were not set."
    );
    // 查一段还没生效的字：照字里新写的档位查。
    let text = CONFIG.replace("reasoning = [\"high\"]", "reasoning = [\"high\", \"max\"]");
    let checked = client
        .call(
            "c",
            "config.check",
            json!({"layer": "system", "text": text}),
        )
        .await;
    assert_eq!(checked["result"]["problems"], json!([]), "{checked}");
}
