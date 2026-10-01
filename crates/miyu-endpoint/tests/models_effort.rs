//! 思考强度在协议上的样子（施工 8-18，`docs/blueprint/models.md`「协议」「怎么走」第十一条）：`session.configure` 的 `effort`
//! 照这时的配置查过记一条（设、清掉、只换强度、和模型一起换、一样的不记），形状不对的 `bad_params`、不是配好的模型
//! `unknown_model`、不在档位里的 `unknown_effort`，哪一样不成都什么都不记；`subscribe` 的 `model` 多 `effort`；`model.list`
//! 的 `facts` 多 `effort`；配置里写的不在档位里的报 `unknown_effort`、算进 `config_errors`。
//!
//! 没有目录：档位全照手写的 `reasoning`。

mod support;

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_endpoint::config::{Config, Environment};
use miyu_kernel::event::{Body, Effort};
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

/// 日志里人换的那几条：换成的模型、思考强度，照先后。
fn changes(home: &Home, session: &str) -> Vec<(Option<String>, Option<Effort>)> {
    home.log(session)
        .into_iter()
        .filter_map(|event| match event.body {
            Body::PolicyChanged(changed) => Some((changed.model, changed.effort)),
            _ => None,
        })
        .collect()
}

fn cell(model: &str, level: Option<&str>) -> Option<Effort> {
    Some(Effort {
        model: model.to_string(),
        level: level.map(str::to_string),
    })
}

#[tokio::test]
async fn session_configure_records_an_effort_for_one_model() {
    let home = Home::new();
    home.write("system/config.toml", CONFIG);
    let mut client = Client::connect(core(&home, false));
    client.hello().await;
    let session = client.create("c1", "/tmp").await;
    client.subscribe("c2", &session).await;
    let set = json!({"session": session, "effort": {"model": "a/m", "level": "high"}});
    let (pushed, reply) = configuring(&mut client, "c3", set.clone()).await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    let event = events(&pushed)
        .into_iter()
        .find(|event| event["kind"] == "session.policy_changed")
        .expect("先推那一条，再回应");
    assert_eq!(
        event["body"],
        json!({"effort": {"model": "a/m", "level": "high"}})
    );
    // 一样的：回 `{}`，什么都不记。
    let (_, reply) = configuring(&mut client, "c4", set).await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    // 和模型一起换；`none` 读成 `off`。
    let both =
        json!({"session": session, "model": "@p", "effort": {"model": "a/m", "level": "none"}});
    let (_, reply) = configuring(&mut client, "c5", both).await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    // 清掉，再清一次。
    for id in ["c6", "c7"] {
        let clear = json!({"session": session, "effort": {"model": "a/m", "level": null}});
        let (_, reply) = configuring(&mut client, id, clear).await;
        assert_eq!(reply["result"], json!({}), "{reply}");
    }
    assert_eq!(
        changes(&home, &session),
        [
            (None, cell("a/m", Some("high"))),
            (Some("@p".to_string()), cell("a/m", Some("off"))),
            (None, cell("a/m", None)),
        ]
    );
}

#[tokio::test]
async fn session_configure_refuses_an_effort_it_cannot_take() {
    let home = Home::new();
    home.write("system/config.toml", CONFIG);
    let mut client = Client::connect(core(&home, false));
    client.hello().await;
    let session = client.create("c1", "/tmp").await;
    let effort = |value: Value| json!({"session": session, "effort": value});
    for (n, params) in [
        effort(json!("high")),
        effort(json!({})),
        effort(json!({"model": "a/m"})),
        effort(json!({"model": "", "level": "high"})),
        effort(json!({"model": 3, "level": "high"})),
        effort(json!({"model": "a/m", "level": ""})),
        effort(json!({"model": "a/m", "level": 3})),
        json!({"session": session, "effort": null}),
        json!({"session": session, "model": "", "effort": {"model": "a/m", "level": "high"}}),
    ]
    .into_iter()
    .enumerate()
    {
        let (_, reply) = configuring(&mut client, &format!("bad-{n}"), params.clone()).await;
        assert_eq!(reason(&reply), Some("bad_params"), "{params}：{reply}");
    }
    for (n, model) in ["@p", "c/m", "nope"].into_iter().enumerate() {
        let params = effort(json!({"model": model, "level": "high"}));
        let (_, reply) = configuring(&mut client, &format!("no-{n}"), params).await;
        assert_eq!(reason(&reply), Some("unknown_model"), "{model}：{reply}");
    }
    for (n, (model, level)) in [("a/m", "max"), ("b/n", "high"), ("a/x", "low")]
        .into_iter()
        .enumerate()
    {
        let params = effort(json!({"model": model, "level": level}));
        let (_, reply) = configuring(&mut client, &format!("level-{n}"), params).await;
        assert_eq!(
            reason(&reply),
            Some("unknown_effort"),
            "{model} {level}：{reply}"
        );
    }
    // 模型换得成、强度不成：整条不成。
    let params =
        json!({"session": session, "model": "@p", "effort": {"model": "a/m", "level": "max"}});
    let (_, reply) = configuring(&mut client, "mixed", params).await;
    assert_eq!(reason(&reply), Some("unknown_effort"), "{reply}");
    assert!(changes(&home, &session).is_empty(), "什么都不记");
}

#[tokio::test]
async fn subscribe_says_which_effort_the_next_request_takes() {
    let home = Home::new();
    home.write("system/config.toml", CONFIG);
    let mut client = Client::connect(core(&home, true));
    client.hello().await;
    for (n, model, wanted) in [
        (
            1,
            "a/m",
            json!({"ref": "a/m", "endpoint": "a", "model": "m", "effort": {"level": "low", "from": "config"}}),
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
async fn model_list_shows_the_default_effort() {
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
        json!({"value": "low", "from": "config", "file": "system/config.toml", "line": line})
    );
    assert_eq!(
        facts("x")["effort"],
        json!({"value": null, "from": "default"}),
        "写的不在档位里：照没写"
    );
    assert_eq!(
        facts("m")["reasoning"]["value"],
        json!(["off", "low", "high"])
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
