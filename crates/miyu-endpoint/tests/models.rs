//! `model.list`（施工 8-7，`docs/blueprint/models.md`「协议」）：形状、每一格的来源、状态；供应商的列表拉完再答、在后台拉；
//! `provider` 只看一家、不是配好了的 `unknown_provider`。目录是真目录裁出来的一份，供应商是本机的假服务器。地址是环境变量
//! 的引用时（施工 8-6b）：`model.list`、`config.get` 都照写的样子交引用，不交解出来的地址。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_endpoint::config::{Config, Environment};
use miyu_http::testkit::{Piece, Reply, Server};
use miyu_http::{Proxy, fetcher};
use miyu_models::catalog::{Catalog, CatalogSource, Loaded};
use miyu_models::matching::Vendors;
use miyu_models::profile::Profiles;
use miyu_models::settings::{
    CatalogSettings, ModelSettings, PriceSettings, ProviderSettings, UseSettings,
};
use miyu_session::testkit::Script;
use miyu_session::{ModelData, Observed};
use miyu_store::resources::ResourceRoot;
use miyu_tool::Catalog as ToolCatalog;

use crate::support::*;

/// 档案：`[npm]` 和 DeepSeek 那一段，照出厂的写。
fn profiles() -> Profiles {
    Profiles::parse(&json!({
        "npm": {"@ai-sdk/openai-compatible": "openai-chat"},
        "providers": {"deepseek": {"driver": "openai-chat", "base_url": "https://api.deepseek.com"}}
    }))
    .expect("档案写法对")
}

/// 带上真目录裁出来的一份、能拉列表的模型资料。
fn data() -> Arc<ModelData> {
    let text = include_str!("../../miyu-models/testdata/models-dev-trimmed.json");
    let vendors = Vendors::parse(&json!({"claude": ["anthropic"], "deepseek": ["deepseek"]}))
        .expect("读得进");
    let data = ModelData::new(profiles(), vendors, None)
        .with_fetcher(fetcher(Proxy::Off).expect("造得出客户端"));
    data.loaded(
        Some(Loaded {
            catalog: Catalog::parse(text).expect("读得进").catalog,
            source: CatalogSource::Snapshot,
            fetched: "2026-10-01T03:25:54.000Z".to_string(),
        }),
        Observed::default(),
    );
    Arc::new(data)
}

/// 三家：只写了 key 的 DeepSeek、地址指到 `relay` 的中转、什么都推不出来的一家。
fn config(relay: &str) -> String {
    format!(
        "[providers.deepseek]\nkey = {{ env = \"DEEPSEEK_API_KEY\" }}\n\n\
         [providers.newapi]\ndriver = \"openai-chat\"\nbase_url = \"{relay}\"\nkey = {{ env = \"NEWAPI_KEY\" }}\nprice_multiplier = 0.5\nlocal = false\n\n\
         [providers.newapi.models.\"claude-sonnet-4-5\"]\nwindow = 100000\n\n\
         [providers.newapi.models.x]\ncatalog = \"deepseek/nope\"\n\n\
         [providers.broken]\nlocal = false\n\n\
         [models]\nchat = \"deepseek/deepseek-flash\"\n"
    )
}

/// 一份核心，清单里带上模型这一块，配置照磁盘上现在的几份读，`env` 是核心的环境，模型资料是 `data`。
fn core(home: &Home, env: &[(&str, &str)], data: Arc<ModelData>) -> Arc<Core> {
    let items = [
        miyu_endpoint::settings::UiSettings::ITEMS,
        UseSettings::ITEMS,
        ProviderSettings::ITEMS,
        ModelSettings::ITEMS,
        PriceSettings::ITEMS,
        CatalogSettings::ITEMS,
    ]
    .concat();
    let config = Config::load(&home.root, &alice(), None, items, Environment::of(env));
    let core = Core::new(
        home.root.clone(),
        ResourceRoot::at(default_resources()),
        Arc::new(Script::new([])),
        ToolCatalog::default(),
        None,
        alice(),
        TOKEN.to_string(),
    );
    Arc::new(core.with_config(config).with_model_data(data))
}

/// 照配置 `config`、环境 `env` 起一个核心，连上、握手，问一次 `model.list`。
async fn list(home: &Home, env: &[(&str, &str)], data: Arc<ModelData>, params: Value) -> Value {
    let core = core(home, env, data);
    let mut client = Client::connect(core);
    client.hello().await;
    client.call("list-1", "model.list", params).await
}

fn model<'a>(provider: &'a Value, name: &str) -> &'a Value {
    provider["models"]
        .as_array()
        .and_then(|models| models.iter().find(|model| model["model"] == name))
        .unwrap_or_else(|| panic!("没有 {name}：{provider}"))
}

#[tokio::test]
async fn the_list_has_providers_models_facts_and_states() {
    let home = Home::new();
    home.write(
        "system/config.toml",
        &config("https://relay.example.invalid/v1"),
    );
    let reply = list(&home, &[("DEEPSEEK_API_KEY", "sk-1")], data(), json!({})).await;
    let result = &reply["result"];
    assert!(result.is_object(), "{reply}");
    assert_eq!(
        result["uses"],
        json!({"chat": "deepseek/deepseek-flash", "vision": null})
    );
    assert_eq!(
        result["catalog"],
        json!({"source": "snapshot", "fetched": "2026-10-01T03:25:54.000Z"})
    );
    let providers = result["providers"].as_array().expect("有供应商");
    let ids: Vec<&str> = providers.iter().filter_map(|p| p["id"].as_str()).collect();
    assert_eq!(ids, ["broken", "deepseek", "newapi"], "照编号排");
    // 推不出来的一家：说清为什么，没有模型。
    assert_eq!(
        providers[0],
        json!({"id": "broken", "name": {"value": "broken", "from": "id", "key": "providers.broken.name"},
               "driver": null, "base_url": null, "models": [],
               "problem": "provider \"broken\" needs base_url: it matches nothing in the catalog"})
    );
    // DeepSeek：档案推出驱动、地址，编号认出目录里的那一家，模型是目录里那一家的四个；key 的值不交出去。
    let deepseek = &providers[1];
    assert_eq!(deepseek["driver"], "openai-chat");
    assert_eq!(
        deepseek["driver_from"], "profile",
        "档案推出来的（施工 8-26）"
    );
    assert_eq!(deepseek["base_url"], "https://api.deepseek.com");
    assert_eq!(
        deepseek["key"],
        json!({"ref": "env:DEEPSEEK_API_KEY", "set": true, "state": "ok"})
    );
    assert_eq!(
        deepseek["catalog"],
        json!({"provider": "deepseek", "how": "id"})
    );
    assert_eq!(
        deepseek["name"],
        json!({"value": "DeepSeek", "from": "catalog", "key": "providers.deepseek.name"})
    );
    let names: Vec<&str> = deepseek["models"]
        .as_array()
        .expect("有模型")
        .iter()
        .filter_map(|m| m["model"].as_str())
        .collect();
    assert_eq!(
        names,
        [
            "deepseek-flash",
            "deepseek-v4-flash",
            "deepseek-v4-flash-vision-exp",
            "deepseek-v4-pro"
        ]
    );
    let from = json!({"from": "catalog", "entry": "deepseek/deepseek-flash", "layer": 2, "fetched": "2026-10-01T03:25:54.000Z"});
    let fact = |value: Value| {
        let mut fact = from.clone();
        fact["value"] = value;
        fact
    };
    assert_eq!(
        model(deepseek, "deepseek-flash"),
        &json!({
            "model": "deepseek-flash",
            "ref": "deepseek/deepseek-flash",
            "listed": ["config", "catalog"],
            "state": "ok",
            "facts": {
                "window": fact(json!(1_000_000)),
                "max_output": fact(json!(393_216)),
                "inputs": fact(json!(["text", "image"])),
                "tools": fact(json!(true)),
                "reasoning": fact(json!(["low", "high", "max"])),
                "effort": {"value": null, "from": "default", "key": "providers.deepseek.models.deepseek-flash.effort"},
                "takes_temperature": fact(json!(true)),
                "temperature": {"value": null, "from": "default", "key": "providers.deepseek.models.deepseek-flash.temperature"},
                "price": fact(json!({"input": 0.15, "output": 0.6, "cache_read": 0.003, "reasoning": 0.6, "currency": "USD"})),
                "multiplier": {"value": 1.0, "from": "default"},
                "name": fact(json!("DeepSeek V4.1 Flash")),
                "status": {"value": null, "from": "default"},
            }
        })
    );
    assert!(!reply.to_string().contains("sk-1"), "key 的值从不交出去");
    // 中转：认不出目录里的哪一家；手写的窗口带文件和行，按名字对上原厂借价，倍率照供应商的；key 一个都没值：no_key。
    let newapi = &providers[2];
    assert!(newapi.get("catalog").is_none());
    let sonnet = model(newapi, "claude-sonnet-4-5");
    assert_eq!(sonnet["listed"], json!(["config"]));
    assert_eq!(sonnet["state"], "no_key");
    assert_eq!(
        sonnet["facts"]["window"],
        json!({"value": 100_000, "from": "config", "file": "system/config.toml", "line": 12, "layer": "system"})
    );
    assert_eq!(sonnet["facts"]["price"]["from"], "catalog");
    assert_eq!(
        sonnet["facts"]["price"]["entry"],
        "anthropic/claude-sonnet-4-5"
    );
    assert_eq!(sonnet["facts"]["price"]["layer"], 3);
    assert_eq!(
        sonnet["facts"]["multiplier"],
        json!({"value": 0.5, "from": "config", "file": "system/config.toml", "line": 8, "layer": "system"})
    );
    // 手写指定的条目不存在：标出来，不借目录。
    let x = model(newapi, "x");
    assert_eq!(x["catalog_missing"], "deepseek/nope");
    assert_eq!(
        x["facts"]["window"],
        json!({"value": null, "from": "default"})
    );
}

/// `model.list` 的 `facts` 里，来源是配置的一格带 `layer`：系统配置写的 `system`，个人设置写的 `personal`，两层都写时跟着
/// 真的来源走（个人设置压着系统配置，施工 8-7（补））。
#[tokio::test]
async fn model_list_facts_say_which_config_layer_won() {
    let home = Home::new();
    home.write(
        "system/config.toml",
        &config("https://relay.example.invalid/v1"),
    );
    home.write(
        "home/alice/settings.toml",
        "[providers.newapi.models.\"claude-sonnet-4-5\"]\nwindow = 222222\n",
    );
    let reply = list(&home, &[], data(), json!({"provider": "newapi"})).await;
    let newapi = &reply["result"]["providers"][0];
    let sonnet = model(newapi, "claude-sonnet-4-5");
    assert_eq!(
        sonnet["facts"]["window"],
        json!({"value": 222_222, "from": "config", "file": "home/alice/settings.toml", "line": 2, "layer": "personal"}),
        "个人设置压着系统配置，layer 跟着换"
    );
    // 只写了系统配置的那一格（倍率）还是 system。
    assert_eq!(sonnet["facts"]["multiplier"]["layer"], "system");
}

#[tokio::test]
async fn one_provider_or_an_unknown_one() {
    let home = Home::new();
    home.write(
        "system/config.toml",
        &config("https://relay.example.invalid/v1"),
    );
    let reply = list(
        &home,
        &[],
        data(),
        json!({"provider": "deepseek", "refresh": null}),
    )
    .await;
    let providers = reply["result"]["providers"].as_array().expect("有");
    assert_eq!(providers.len(), 1);
    assert_eq!(providers[0]["id"], "deepseek");
    assert_eq!(
        model(&providers[0], "deepseek-flash")["state"],
        "no_key",
        "写了 key、一个都没值"
    );
    let reply = list(&home, &[], data(), json!({"provider": "nope"})).await;
    assert_eq!(reason(&reply), Some("unknown_provider"));
    let reply = list(&home, &[], data(), json!({"provider": 7})).await;
    assert_eq!(reason(&reply), Some("bad_params"));
}

/// 供应商的列表：`refresh` 拉完再答（带认证头、GET `/models`），窗口照列表的；不写 `refresh` 的在后台拉、这一次先照手头的。
#[tokio::test]
async fn the_provider_list_is_fetched_now_or_in_the_background() {
    let page =
        r#"{"data":[{"id":"claude-sonnet-4-5","context_length":150000},{"id":"only-here"}]}"#;
    let reply = || Reply::stream(vec![Piece::Bytes(page.as_bytes().to_vec())]);
    let server = Server::start(vec![reply(), reply()]).await;
    let home = Home::new();
    home.write("system/config.toml", &config(&server.base_url));
    let env = [("NEWAPI_KEY", "sk-relay")];
    let shared = data();
    let answer = list(
        &home,
        &env,
        Arc::clone(&shared),
        json!({"provider": "newapi", "refresh": true}),
    )
    .await;
    let received = server.received();
    assert_eq!(received.len(), 1);
    assert_eq!(
        (received[0].method.as_str(), received[0].path.as_str()),
        ("GET", "/v1/models")
    );
    assert_eq!(received[0].header("authorization"), Some("Bearer sk-relay"));
    let newapi = &answer["result"]["providers"][0];
    let only = model(newapi, "only-here");
    assert_eq!(only["listed"], json!(["provider"]));
    assert_eq!(only["state"], "ok");
    let sonnet = model(newapi, "claude-sonnet-4-5");
    assert_eq!(sonnet["listed"], json!(["config", "provider"]));
    assert_eq!(
        sonnet["facts"]["window"]["from"], "config",
        "手写的盖过列表"
    );
    assert!(shared.list_fetched("newapi").is_some(), "拉到的换上了");
    // 另一份模型资料、没拉过：不写 refresh 的这一次照手头的答，在后台拉。
    let fresh = data();
    let answer = list(
        &home,
        &env,
        Arc::clone(&fresh),
        json!({"provider": "newapi"}),
    )
    .await;
    assert!(
        model(&answer["result"]["providers"][0], "claude-sonnet-4-5")["listed"]
            == json!(["config"]),
        "这一次先照手头的"
    );
    let waited = tokio::time::timeout(Duration::from_secs(10), async {
        while fresh.list_fetched("newapi").is_none() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
    assert!(waited.is_ok(), "在后台拉了");
    assert_eq!(server.received().len(), 2);
}

/// 地址是环境变量的引用（施工 8-6b）：`model.list`、`config.get` 都照写的样子交引用，整份回应里搜不到解出来的地址；
/// 环境变量没设的报 `env_not_set` 警告。
#[tokio::test]
async fn an_env_based_address_never_leaves_model_list_or_config_get() {
    let home = Home::new();
    home.write(
        "system/config.toml",
        "[providers.relay]\ndriver = \"openai-chat\"\nbase_url = { env = \"RELAY_URL\" }\n\n[models]\nchat = \"relay/m\"\n",
    );
    let secret_address = "https://secret-relay.example.invalid/v1";
    let with_env = core(&home, &[("RELAY_URL", secret_address)], data());
    let mut client = Client::connect(with_env);
    client.hello().await;
    let get = client.call("get-1", "config.get", json!({})).await;
    assert!(!get.to_string().contains(secret_address), "{get}");
    assert_eq!(
        get["result"]["items"]["providers.relay.base_url"]["value"],
        json!({"env": "RELAY_URL"})
    );
    assert_eq!(get["result"]["problems"], json!([]), "设了就不报警告");
    let list = client.call("list-1", "model.list", json!({})).await;
    assert!(!list.to_string().contains(secret_address), "{list}");
    let relay = list["result"]["providers"]
        .as_array()
        .and_then(|providers| providers.iter().find(|p| p["id"] == "relay"))
        .unwrap_or_else(|| panic!("没有 relay：{list}"));
    assert_eq!(relay["base_url"], json!({"env": "RELAY_URL"}));

    // 没设：config.get 报 env_not_set 警告，base_url 还是引用，不是 null。
    let without_env = core(&home, &[], data());
    let mut client = Client::connect(without_env);
    client.hello().await;
    let get = client.call("get-1", "config.get", json!({})).await;
    assert_eq!(
        get["result"]["items"]["providers.relay.base_url"]["value"],
        json!({"env": "RELAY_URL"})
    );
    assert_eq!(get["result"]["problems"][0]["code"], "env_not_set", "{get}");
}

/// 地址是环境变量的引用时拉供应商的模型列表（施工 8-6b，`route/lists.rs`）：`refresh` 照样连得上假服务器，地址不进回应。
#[tokio::test]
async fn fetching_the_provider_list_resolves_an_env_based_address() {
    let page = r#"{"data":[{"id":"only-here"}]}"#;
    let server = Server::start(vec![Reply::stream(vec![Piece::Bytes(
        page.as_bytes().to_vec(),
    )])])
    .await;
    let home = Home::new();
    home.write(
        "system/config.toml",
        "[providers.relay]\ndriver = \"openai-chat\"\nbase_url = { env = \"RELAY_URL\" }\n\n[models]\nchat = \"relay/m\"\n",
    );
    let reply = list(
        &home,
        &[("RELAY_URL", &server.base_url)],
        data(),
        json!({"provider": "relay", "refresh": true}),
    )
    .await;
    let received = server.received();
    assert_eq!(received.len(), 1, "照环境变量取的地址连上了假服务器");
    assert_eq!(received[0].path, "/v1/models");
    let relay = &reply["result"]["providers"][0];
    assert_eq!(relay["base_url"], json!({"env": "RELAY_URL"}));
    assert!(!reply.to_string().contains(&server.base_url), "{reply}");
}

/// 冷却（施工 8-9，`models.md`「协议」`model.list` 的 `state`；施工 8-25 起一家一个 key）：限速停这一家的这个模型，模型是
/// `cooling`，带 `until`、`class`，别的模型照常；认证失败停整家，key 和这一家的每个模型都是 `cooling`、`class` 是 `auth`。
/// 写了 key、取不到值的，模型都是 `no_key`。
#[tokio::test]
async fn cooling_shows_on_models_and_the_key() {
    use miyu_kernel::event::ErrorClass;
    use miyu_models::cooldown::Candidate;

    let home = Home::new();
    home.write(
        "system/config.toml",
        &config("https://relay.example.invalid/v1"),
    );
    let data = data();
    let now = wall_now();
    let fail = |model: &str, class: ErrorClass| {
        data.cooldown(|table, rules| {
            table
                .fail(&Candidate::new("deepseek", model), &class, None, rules, now)
                .expect("记了")
                .until
        })
    };
    let set = [("DEEPSEEK_API_KEY", "sk-1")];
    let limited = fail("deepseek-flash", ErrorClass::RateLimited);
    let reply = list(
        &home,
        &set,
        Arc::clone(&data),
        json!({"provider": "deepseek"}),
    )
    .await;
    let deepseek = &reply["result"]["providers"][0];
    assert_eq!(
        deepseek["key"],
        json!({"ref": "env:DEEPSEEK_API_KEY", "set": true, "state": "ok"}),
        "限速只停这个模型"
    );
    let flash = model(deepseek, "deepseek-flash");
    assert_eq!(
        (&flash["state"], &flash["until"], &flash["class"]),
        (&json!("cooling"), &json!(limited), &json!("rate_limited"))
    );
    assert_eq!(model(deepseek, "deepseek-v4-pro")["state"], "ok");

    let stopped = fail("deepseek-v4-pro", ErrorClass::Auth);
    let reply = list(
        &home,
        &set,
        Arc::clone(&data),
        json!({"provider": "deepseek"}),
    )
    .await;
    let deepseek = &reply["result"]["providers"][0];
    assert_eq!(
        deepseek["key"],
        json!({"ref": "env:DEEPSEEK_API_KEY", "set": true, "state": "cooling", "until": stopped, "class": "auth"}),
        "认证失败停了整家"
    );
    let flash_now = model(deepseek, "deepseek-v4-flash");
    assert_eq!(
        (&flash_now["state"], &flash_now["class"]),
        (&json!("cooling"), &json!("auth")),
        "这一家别的模型也停"
    );

    let reply = list(&home, &[], data, json!({"provider": "deepseek"})).await;
    let deepseek = &reply["result"]["providers"][0];
    assert_eq!(deepseek["key"]["set"], false);
    assert_eq!(model(deepseek, "deepseek-flash")["state"], "no_key");
}

/// 现在，照系统时间。
fn wall_now() -> miyu_kernel::time::Timestamp {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("系统时间在 1970 年以后")
        .as_millis();
    miyu_kernel::time::Timestamp::from_unix_millis(i64::try_from(millis).expect("放得下"))
        .expect("在范围里")
}
