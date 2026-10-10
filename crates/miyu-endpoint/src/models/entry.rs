//! `model.list` 里的一家（`docs/blueprint/models.md`「协议」`model.list` 那张表，施工 8-7）：驱动、地址、key、对上了目录里的
//! 哪一家、模型。
//!
//! - 列哪些模型：供应商的列表里的、目录里对上的那一家的、配置里手写了的、用途池里点名的（施工 8-8），合在一起去重，
//!   照模型名排；
//!   `listed` 照 `config`、`provider`、`catalog` 的先后写从哪几处列出来的。
//! - 模型的 `state`：写了 key、一个都没有值的是 `no_key`。别的照这个模型能用的 key（取得到值的，没写 key 的是那一个）里
//!   最好的那个：有一个不在冷却就是 `ok`；都在冷却的是 `cooling`，带最早恢复的那一个的 `until`、`class`（施工 8-9，整个 key
//!   在冷却、这个 key 的这个模型在冷却都算，取晚的）。key 的 `state` 是 `ok`，或者认证失败停了整个 key 的 `cooling`，带
//!   `until`、`class`。
//! - 这一家用不了（推不出驱动、地址，驱动还没有）：驱动、地址照手写的写，没写的是 `null`，带上 `problem` 那一句，没有模型。
//! - `base_url` 照配置写的样子交（`address_json`，施工 8-6b）：写死的是地址本身，是环境变量的引用的交 `{"env": "…"}`，
//!   地址本身不解出来，不会进这份回应。
//! - 思考强度的那一格多 `key`（施工 8-18（补），`models.md`「协议」）：这一项完整的配置键名，头照抄它发 `config.set`。
//! - 显示名 `name`（施工 8-21）：写了的照写的（带文件、行、层），只有空白的当没写；没写的、对上了目录的照目录里那一家
//!   的名字；都没有的照编号。用不了的那一家也有。也带 `key`。
//! - 能出向量的（施工 R-5 再补）：照 `names`（资源目录的 `models/embedding.toml`）认模型名和目录里那个条目的 `family`，认得出
//!   的多一格 `embedding: true`，不是的不写。

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

use miyu_config::Address;
use miyu_config::Value as ConfigValue;
use miyu_config::key as config_key;
use miyu_config::merge::Origin;
use miyu_kernel::time::Timestamp;
use miyu_models::cooldown::{Candidate, Cooling};
use miyu_models::effort;
use miyu_models::embedding::EmbeddingNames;
use miyu_models::facts::facts;
use miyu_models::matching::Found;
use miyu_models::provider::{self, NoModel, key_name};
use miyu_models::reference::named;
use miyu_models::settings::ProviderSettings;
use miyu_models::temperature;
use miyu_session::ModelData;

use super::Snapshot;

/// 编号 `id` 这一家，冷却照 `now` 这一刻，能出向量的照 `names` 认。
pub(crate) fn provider(
    data: &ModelData,
    snapshot: &Snapshot,
    id: &str,
    now: Timestamp,
    names: &EmbeddingNames,
) -> Value {
    let values = snapshot.resolved.values();
    let settings = ProviderSettings::at(&values, &[id]);
    // 这一家的 key（施工 8-25：一家一个）：引用的写法、取不取得到、这一家整个冷不冷（认证失败停的）。没写的没有这一格。
    let key: Option<Value> = settings.key.as_ref().map(|reference| {
        let cooling = data.cooldown(|table, _| table.provider_cooling(id, now));
        let mut key = json!({
            "ref": key_name(reference),
            "set": snapshot.secret(reference).is_some(),
        });
        state(&mut key, cooling.as_ref());
        key
    });
    // 写了 key、取不到值的：这一家的模型都是 `no_key`。
    let no_key = settings
        .key
        .as_ref()
        .is_some_and(|reference| snapshot.secret(reference).is_none());
    data.with(
        |knowledge| match provider::provider(&values, knowledge, id) {
            Err(NoModel(problem)) => {
                let mut entry = json!({
                    "id": id,
                    "name": name(snapshot, id, None),
                    "driver": settings.driver,
                    "base_url": settings.base_url.as_ref().map(address_json),
                    "problem": problem,
                    "models": [],
                    "logo": null,
                });
                if let Some(key) = &key {
                    entry["key"] = key.clone();
                }
                entry
            }
            Ok(found) => {
                let mut listed: BTreeMap<String, BTreeSet<&str>> = BTreeMap::new();
                for model in written_models(&values, id) {
                    listed.entry(model).or_default().insert("config");
                }
                if let Some(list) = knowledge.lists.get(id) {
                    for model in &list.models {
                        listed
                            .entry(model.id.clone())
                            .or_default()
                            .insert("provider");
                    }
                }
                let recognized = found.recognized.as_ref();
                let catalog_models = recognized
                    .zip(knowledge.catalog)
                    .and_then(|(recognized, loaded)| loaded.catalog.provider(&recognized.provider));
                if let Some(entry) = catalog_models {
                    for model in entry.models.keys() {
                        listed.entry(model.clone()).or_default().insert("catalog");
                    }
                }
                let models: Vec<Value> = listed
                    .into_iter()
                    .map(|(model, from)| {
                        let (facts, matched) = facts(&snapshot.resolved, knowledge, &found, &model);
                        let places: Vec<&str> = ["config", "provider", "catalog"]
                            .into_iter()
                            .filter(|place| from.contains(place))
                            .collect();
                        let mut entry = json!({
                            "model": model,
                            "ref": format!("{id}/{model}"),
                            "listed": places,
                            "facts": facts.json(&|layer| snapshot.file(layer)),
                        });
                        entry["facts"]["effort"]["key"] =
                            json!(config_key::fill(effort::ITEM, &[id, &model]));
                        entry["facts"]["temperature"]["key"] =
                            json!(config_key::fill(temperature::ITEM, &[id, &model]));
                        match no_key {
                            true => entry["state"] = json!("no_key"),
                            false => {
                                let candidate = Candidate::new(id, &model);
                                let cooling =
                                    data.cooldown(|table, _| table.cooling(&candidate, now));
                                state(&mut entry, cooling.as_ref());
                            }
                        }
                        if let Found::Missing(missing) = matched {
                            entry["catalog_missing"] = json!(missing);
                        }
                        let family = catalog_models
                            .and_then(|listed| listed.models.get(&model))
                            .and_then(|known| known.family.as_deref());
                        if names.matches(&model, family) {
                            entry["embedding"] = json!(true);
                        }
                        entry
                    })
                    .collect();
                let driver = found.driver.as_str();
                let catalog_name = recognized
                    .zip(knowledge.catalog)
                    .and_then(|(recognized, loaded)| loaded.catalog.provider(&recognized.provider))
                    .and_then(|entry| entry.name.as_deref());
                let mut entry = json!({
                    "id": id,
                    "name": name(snapshot, id, catalog_name),
                    "driver": driver,
                    "driver_from": found.driver_from.as_str(),
                    "base_url": address_json(&found.base_url),
                    "models": models,
                });
                if let Some(key) = &key {
                    entry["key"] = key.clone();
                }
                if let Some(recognized) = recognized {
                    entry["catalog"] =
                        json!({"provider": recognized.provider, "how": recognized.how.as_str()});
                }
                // 图标照认出来的目录里那一家（施工 8-31）；认不出的（自定义、本机服务）是 null。
                entry["logo"] =
                    crate::providers::logo(data, recognized.map(|known| known.provider.as_str()));
                entry
            }
        },
    )
}

/// 这一家给人看的名字：写了的（去掉两头空白不是空的）、目录里那一家的、编号，照这个先后（施工 8-21）。
fn name(snapshot: &Snapshot, id: &str, catalog: Option<&str>) -> Value {
    let key = config_key::fill(NAME, &[id]);
    let written = snapshot
        .resolved
        .get(&key)
        .and_then(|(value, origin)| match (value, origin) {
            (ConfigValue::Text(text), Origin::File { layer, line }) if !text.trim().is_empty() => {
                Some(json!({"value": text.trim(), "from": "config", "file": snapshot.file(*layer), "line": line, "layer": layer.as_str()}))
            }
            _ => None,
        });
    let mut name = written.unwrap_or_else(|| match catalog {
        Some(name) => json!({"value": name, "from": "catalog"}),
        None => json!({"value": id, "from": "id"}),
    });
    name["key"] = json!(key);
    name
}

/// 显示名在清单里的键。
const NAME: &str = "providers.<id>.name";

/// 写上状态：不在冷却的 `ok`；在冷却的 `cooling`，带 `until`、`class`（施工 8-9）。
fn state(entry: &mut Value, cooling: Option<&Cooling>) {
    match cooling {
        None => entry["state"] = json!("ok"),
        Some(cooling) => {
            entry["state"] = json!("cooling");
            entry["until"] = json!(cooling.until);
            entry["class"] = json!(cooling.class.as_str());
        }
    }
}

/// 地址照配置写的样子交：写死的就是地址本身，引用就交引用（`{"env": "…"}`），不交解出来的地址（施工 8-6b）。
fn address_json(address: &Address) -> Value {
    match address {
        Address::Literal(text) => json!(text),
        Address::Env(name) => miyu_config::secret::Reference::Env(name.clone()).json(),
    }
}

/// 配置里提到的这一家的模型：手写了资料的，用途、池里点名的（施工 8-8，`miyu_models::reference::named`）。
fn written_models(values: &miyu_config::Values, id: &str) -> Vec<String> {
    let mut models = miyu_config::key::names(values.keys(), "providers.<id>.models.<model>", &[id]);
    models.extend(
        named(values)
            .into_iter()
            .filter(|(provider, _)| provider == id)
            .map(|(_, model)| model),
    );
    models
}
