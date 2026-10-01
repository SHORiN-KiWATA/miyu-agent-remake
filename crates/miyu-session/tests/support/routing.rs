//! 经路由请求假服务器（施工 8-6）：配置照系统配置的字读、合，供应商的地址指到假服务器，key 由测试给。模型资料（施工 8-7）
//! 默认没有目录、读完了。

use std::sync::Arc;
use std::time::Duration;

use miyu_config::merge::{Layers, merge};
use miyu_config::parse::parse;
use miyu_config::secret::{Reference, Secret};
use miyu_config::{Item, Layer};
use miyu_http::{Proxy, client};
use miyu_models::matching::Vendors;
use miyu_models::profile::Profiles;
use miyu_models::settings::{ModelSettings, PriceSettings, ProviderSettings, UseSettings};
use miyu_session::{Configs, ModelData, Observed, Routes, fixed_with};

/// 模型这一块的配置项。
pub fn items() -> Vec<Item> {
    [
        ProviderSettings::ITEMS,
        ModelSettings::ITEMS,
        PriceSettings::ITEMS,
        UseSettings::ITEMS,
    ]
    .concat()
}

/// 照系统配置的字 `source` 造一份不变的配置，另带取得到的几个密钥：引用和值。写错的配置当场报出来。
pub fn configs(source: &str, secrets: &[(Reference, &str)]) -> Configs {
    let parsed = parse(&items(), Layer::System, source).expect("写法对");
    assert!(parsed.problems.is_empty(), "{:?}", parsed.problems);
    let layers = Layers {
        system: Some(&parsed),
        ..Layers::default()
    };
    let secrets = secrets
        .iter()
        .map(|(reference, value)| (reference.clone(), Secret::new(value).expect("key 合写法")))
        .collect();
    fixed_with(merge(&items(), &layers, &|_| None), secrets)
}

/// 核心一份的：档案照 JSON 的 `profiles`，没有目录（读完了），空闲超时 `idle`。
pub fn routes(profiles: serde_json::Value, idle: Duration) -> Routes {
    let data = ModelData::new(
        Profiles::parse(&profiles).expect("档案写法对"),
        Vendors::default(),
        None,
    );
    data.loaded(None, Observed::default());
    routes_with(Arc::new(data), idle)
}

/// 同上，模型资料照 `data`。
pub fn routes_with(data: Arc<ModelData>, idle: Duration) -> Routes {
    Routes {
        client: client(Proxy::Off).expect("造得出客户端"),
        data,
        idle,
    }
}

/// 发给假服务器 `base_url` 的一家 `deepseek`：OpenAI 兼容的写法，模型 `deepseek-v4`，key 是 `sk-test`（`{ env = "TEST_KEY" }`），
/// `models.chat` 指着它，模型手写的资料另写 `model` 那几行。档案里 `deepseek` 那一段是 `profile`，空闲超时五秒。
pub fn served(base_url: &str, profile: serde_json::Value, model: &str) -> (Routes, Configs) {
    let source = format!(
        "[providers.deepseek]\ndriver = \"openai-chat\"\nbase_url = \"{base_url}\"\nkeys = [{{ env = \"TEST_KEY\" }}]\n\n[providers.deepseek.models.\"deepseek-v4\"]\n{model}\n[models]\nchat = \"deepseek/deepseek-v4\"\n"
    );
    let configs = configs(
        &source,
        &[(Reference::Env("TEST_KEY".to_string()), "sk-test")],
    );
    let routes = routes(
        serde_json::json!({"providers": {"deepseek": profile}}),
        Duration::from_secs(5),
    );
    (routes, configs)
}
