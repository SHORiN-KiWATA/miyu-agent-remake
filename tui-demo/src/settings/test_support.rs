//! 配置页测试用的样例：两家供应商（一家中转站带组织前缀）、两个池、默认用途，个人层写着的几项，密钥库的名字。

use serde_json::{Value, json};

use super::data::Data;

/// `model.list` 的样子（照核心 8-7、8-8、8-18、8-21、8-22 的形状，只留配置页读的几格）。
pub fn model_list() -> Value {
    json!({
        "uses": {"chat": "@daily", "vision": "relay/zhipu/glm-5v"},
        "pools": [
            {"name": "daily", "strategy": "rotate", "models": ["dev/flash", "relay/cline/deepseek-v4"]},
            {"name": "empty", "strategy": "pin", "models": []}
        ],
        "providers": [
            {"id": "dev", "name": {"value": "dev", "from": "id"}, "driver": "openai-chat",
             "base_url": {"env": "MIYU_DEV_BASE_URL"},
             "key": {"ref": "env:DEEPSEEK_API_KEY", "set": true, "state": "ok"},
             "models": [
                {"model": "flash", "ref": "dev/flash", "listed": ["config"],
                 "facts": {
                    "window": {"value": 128000, "from": "config", "layer": "personal"},
                    "inputs": {"value": ["text"], "from": "default"},
                    "reasoning": {"value": ["off", "high"], "from": "catalog"},
                    "effort": {"value": null, "from": "default", "key": "providers.dev.models.flash.effort"},
                    "temperature": {"value": null, "from": "default", "key": "providers.dev.models.flash.temperature"},
                    "takes_temperature": {"value": null, "from": "default"},
                    "price": {"value": null, "from": "default"},
                    "multiplier": {"value": 1.0, "from": "default"}}}
             ]},
            {"id": "relay", "name": {"value": "中转站", "from": "config"}, "driver": "openai-chat",
             "base_url": "https://relay.example.invalid/v1",
             "key": {"ref": "secret:relay-key", "set": true, "state": "ok"},
             "models": [
                {"model": "cline/deepseek-v4", "ref": "relay/cline/deepseek-v4", "listed": ["provider", "catalog"],
                 "facts": {
                    "window": {"value": 1000000, "from": "catalog"},
                    "inputs": {"value": ["text"], "from": "catalog"},
                    "effort": {"value": null, "from": "default", "key": "providers.relay.models.\"cline/deepseek-v4\".effort"},
                    "takes_temperature": {"value": true, "from": "catalog"},
                    "price": {"value": {"input": 0.27, "output": 1.1, "cache_read": 0.07, "currency": "USD"}, "from": "catalog"}}},
                {"model": "zhipu/glm-5v", "ref": "relay/zhipu/glm-5v", "listed": ["provider"],
                 "facts": {
                    "window": {"value": 64000, "from": "provider"},
                    "inputs": {"value": ["text", "image"], "from": "catalog"},
                    "takes_temperature": {"value": false, "from": "catalog"},
                    "effort": {"value": null, "from": "default", "key": "providers.relay.models.\"zhipu/glm-5v\".effort"}}},
                {"model": "gpt-oss", "ref": "relay/gpt-oss", "listed": ["provider"],
                 "facts": {"window": {"value": 128000, "from": "provider"},
                           "inputs": {"value": ["text"], "from": "default"},
                           "effort": {"value": null, "from": "default", "key": "providers.relay.models.gpt-oss.effort"}}}
             ]}
        ]
    })
}

/// `config.get`（`all: true`）的样子：个人层写了 dev 的窗口、relay 的显示名和 key、默认用途；系统层写了 relay 的地址。
pub fn config_all() -> Value {
    let personal =
        |v: Value| json!({"origin": {"layer": "personal", "line": 1}, "value": v, "used": true});
    let system =
        |v: Value| json!({"origin": {"layer": "system", "line": 1}, "value": v, "used": true});
    json!({"items": {
        "providers.dev.models.flash.window": {"layers": [personal(json!(128000))]},
        "providers.relay.name": {"layers": [personal(json!("中转站"))]},
        "providers.relay.key": {"layers": [personal(json!({"secret": "relay-key"}))]},
        "providers.relay.base_url": {"layers": [system(json!("https://relay.example.invalid/v1"))]},
        "models.chat": {"layers": [personal(json!("@daily"))]},
        "pools.daily.models": {"layers": [personal(json!(["dev/flash", "relay/cline/deepseek-v4"]))]},
        "pools.daily.strategy": {"layers": [personal(json!("rotate"))]},
        "pools.empty.models": {"layers": [personal(json!([]))]}
    }})
}

/// 读好的一份。
pub fn sample() -> Data {
    let mut data = Data::default();
    data.read_models(&model_list());
    data.read_config(&config_all());
    data.read_secrets(&json!({"secrets": [{"name": "relay-key", "set": true}]}));
    data
}
