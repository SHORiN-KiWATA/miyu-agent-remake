//! 第一次接入的测试共用的（施工 8-11）：带真目录裁出来的一份、能探本机、能拉列表的模型资料，清单里带上模型这一块的核心。
//! `model.call` 的测试（施工 8-20）另用请求模型的是真路由的核心（[`routed`]）、说一句话的流（[`said`]）。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_endpoint::config::{Config, Environment};
use miyu_http::testkit::{Piece, Reply};
use miyu_http::{Proxy, client, fetcher};
use miyu_models::catalog::{Catalog, CatalogSource, Loaded};
use miyu_models::matching::Vendors;
use miyu_models::profile::Profiles;
use miyu_models::settings::{
    CatalogSettings, ModelSettings, PoolSettings, PriceSettings, ProviderSettings, UsageSettings,
    UseSettings,
};
use miyu_session::testkit::Script;
use miyu_session::{ModelData, Models, Observed, Routes};
use miyu_store::resources::ResourceRoot;
use miyu_tool::Catalog as ToolCatalog;

use super::{Home, TOKEN, alice, default_resources};

/// 档案：出厂的 `[npm]`、DeepSeek 那一段，加上 `extra` 里的几家（编号 → 档案）。
pub fn profiles(extra: Value) -> Profiles {
    let mut providers = json!({
        "deepseek": {"driver": "openai-chat", "base_url": "https://api.deepseek.com"}
    });
    if let (Some(all), Value::Object(more)) = (providers.as_object_mut(), extra) {
        all.extend(more);
    }
    Profiles::parse(&json!({
        "npm": {
            "@ai-sdk/openai-compatible": "openai-chat",
            "@ai-sdk/anthropic": "anthropic",
            "@ai-sdk/openai": "openai-responses"
        },
        "providers": providers,
    }))
    .expect("档案写法对")
}

/// 带上真目录裁出来的一份、档案 `profiles`，能拉列表、能探本机（都不走代理）的模型资料。
pub fn data(profiles: Profiles) -> Arc<ModelData> {
    let text = include_str!("../../../miyu-models/testdata/models-dev-trimmed.json");
    let vendors = Vendors::parse(&json!({"claude": ["anthropic"], "deepseek": ["deepseek"]}))
        .expect("读得进");
    let data = ModelData::new(profiles, vendors, None)
        .with_fetcher(fetcher(Proxy::Off).expect("造得出客户端"))
        .with_local(fetcher(Proxy::Off).expect("造得出客户端"));
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

/// 一份核心：清单里带上模型这一块，配置照磁盘上现在的几份读，`env` 是核心的环境，模型资料是 `data`。
pub fn core(home: &Home, env: &[(&str, &str)], data: Arc<ModelData>) -> Arc<Core> {
    assembled(home, env, data, Arc::new(Script::new([])))
}

/// 同 [`core`]，请求模型的是真的路由，模型资料是同一份 `data`（施工 8-20：`model.call` 经它的一次性入口）。
pub fn routed(home: &Home, env: &[(&str, &str)], data: Arc<ModelData>) -> Arc<Core> {
    let routes = Routes {
        client: client(Proxy::Off).expect("造得出客户端"),
        direct: client(Proxy::Off).expect("造得出客户端"),
        data: Arc::clone(&data),
        idle: Duration::from_secs(60),
    };
    assembled(home, env, data, Arc::new(routes))
}

/// 同 [`core`]，请求模型照剧本 `script`（施工 8-15：剧本带价格，会话的请求记金额）。
pub fn scripted(home: &Home, data: Arc<ModelData>, script: Script) -> Arc<Core> {
    assembled(home, &[], data, Arc::new(script))
}

/// 说 `text` 的一份流：一段正文、说完、用量（输入 12，输出 3）。
pub fn said(text: &str) -> Reply {
    let chunk = |choices: Value, usage: Value| {
        let event = json!({"id": "c1", "object": "chat.completion.chunk", "model": "x",
            "choices": choices, "usage": usage});
        format!("data: {event}\n\n")
    };
    let body = [
        chunk(
            json!([{"index": 0, "delta": {"role": "assistant", "content": text}, "finish_reason": null}]),
            Value::Null,
        ),
        chunk(
            json!([{"index": 0, "delta": {}, "finish_reason": "stop"}]),
            Value::Null,
        ),
        chunk(
            json!([]),
            json!({"prompt_tokens": 12, "completion_tokens": 3, "total_tokens": 15}),
        ),
        "data: [DONE]\n\n".to_string(),
    ]
    .concat();
    Reply::stream(vec![Piece::Bytes(body.into_bytes())])
}

/// 一份核心，请求模型照 `models`。
fn assembled(
    home: &Home,
    env: &[(&str, &str)],
    data: Arc<ModelData>,
    models: Arc<dyn Models>,
) -> Arc<Core> {
    let items = [
        miyu_endpoint::settings::UiSettings::ITEMS,
        UseSettings::ITEMS,
        ProviderSettings::ITEMS,
        ModelSettings::ITEMS,
        PriceSettings::ITEMS,
        CatalogSettings::ITEMS,
        PoolSettings::ITEMS,
        UsageSettings::ITEMS,
    ]
    .concat();
    let config = Config::load(&home.root, &alice(), None, items, Environment::of(env));
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
