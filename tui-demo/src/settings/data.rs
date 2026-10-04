//! 核心的模型列表与配置快照：不建立第二份模型白名单，不读写磁盘。

use serde_json::{Value, json};

/// 一个供应商的列表资料。
#[derive(Clone)]
pub struct Provider {
    /// 配置编号。
    pub id: String,
    /// 核心的显示名，没有时用编号。
    pub name: String,
    /// 核心列的全部模型。
    pub models: Vec<Model>,
}

/// 一个模型和最终的资料。
#[derive(Clone)]
pub struct Model {
    /// 供应商给的名字。
    pub id: String,
    /// 完整引用。
    pub reference: String,
    /// 核心按来源合成的资料。
    pub facts: Value,
}

/// 一个自定义模型池。
#[derive(Clone)]
pub struct Pool {
    /// 配置名称。
    pub name: String,
    /// 调用方式。
    pub strategy: String,
    /// 成员引用，保留配置先后。
    pub models: Vec<String>,
}

/// 从 model.list 读取供应商；有模型字段但没有显示名的仍列出，不按 key/冷却筛掉。
pub fn providers(value: &Value) -> Vec<Provider> {
    value["providers"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| {
            let id = p["id"].as_str()?.to_string();
            Some(Provider {
                name: p["name"].as_str().unwrap_or(&id).to_string(),
                models: p["models"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|m| {
                        Some(Model {
                            id: m["model"].as_str()?.to_string(),
                            reference: m["ref"].as_str()?.to_string(),
                            facts: m["facts"].clone(),
                        })
                    })
                    .collect(),
                id,
            })
        })
        .collect()
}

/// 从 model.list 读取池，包括空池和暂时失效的成员，不自行清理配置。
pub fn pools(value: &Value) -> Vec<Pool> {
    value["pools"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| {
            Some(Pool {
                name: p["name"].as_str()?.to_string(),
                strategy: p["strategy"].as_str().unwrap_or("pin").to_string(),
                models: p["models"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect(),
            })
        })
        .collect()
}

/// 模型的输入种类；未知能力不推断成图像。
pub fn inputs(model: &Model) -> Vec<String> {
    model.facts["inputs"]["value"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect()
}

/// 改动的个人层预期；系统层最终值不当成个人层的原值。
pub fn expected(config: &Value, key: &str) -> Value {
    let item = &config["items"][key];
    let personal = item["layers"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|l| l["origin"]["layer"] == "personal");
    match personal {
        Some(layer) => json!({"value": layer["value"]}),
        None if item["origin"]["layer"] == "personal" => json!({"value": item["value"]}),
        _ => json!({}),
    }
}

/// 为一项配置补个人层 expect，不改变 value/input/unset。
pub fn guarded(config: &Value, mut change: Value) -> Value {
    if let Some(key) = change["key"].as_str() {
        change["expect"] = expected(config, key);
    }
    change
}

/// 核心配置 get 的最终值。
pub fn value<'a>(config: &'a Value, key: &str) -> &'a Value {
    &config["items"][key]["value"]
}

/// 短名字的配置键段：只在需要时引号包住，和核心 key::fill 的样子一致。
pub fn segment(name: &str) -> String {
    if !name.is_empty()
        && name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
    {
        name.to_string()
    } else {
        // 序列化字符串不可能失败。
        serde_json::to_string(name).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_model_is_listed_even_when_a_key_is_missing() {
        let list = providers(&json!({"providers":[{"id":"p","models":[
            {"model":"a","ref":"p/a","state":"no_key","facts":{}},
            {"model":"b","ref":"p/b","state":"cooling","facts":{"inputs":{"value":["image"]}}}]}]}));
        assert_eq!(list[0].name, "p");
        assert_eq!(list[0].models.len(), 2);
        assert_eq!(inputs(&list[0].models[1]), ["image"]);
    }

    #[test]
    fn conflict_guard_uses_the_personal_layer_even_when_system_is_effective() {
        let config = json!({"items":{"x":{"value":8,"origin":{"layer":"system"},
            "layers":[{"value":3,"origin":{"layer":"personal"}},{"value":8,"origin":{"layer":"system"}}]}}});
        assert_eq!(expected(&config, "x"), json!({"value":3}));
        assert_eq!(expected(&config, "absent"), json!({}));
    }

    #[test]
    fn dotted_model_names_are_single_key_segments_and_pools_keep_unknown_members() {
        assert_eq!(segment("a.b"), "\"a.b\"");
        assert_eq!(segment("a-b"), "a-b");
        let p =
            pools(&json!({"pools":[{"name":"daily","strategy":"pin","models":["gone/model"]}]}));
        assert_eq!(p[0].models, ["gone/model"]);
    }
}
