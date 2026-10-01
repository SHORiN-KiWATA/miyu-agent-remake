//! `model.list` 里的一家（`docs/blueprint/models.md`「协议」`model.list` 那张表，施工 8-7）：驱动、地址、key、对上了目录里的
//! 哪一家、模型。
//!
//! - 列哪些模型：供应商的列表里的、目录里对上的那一家的、配置里手写了的、`models.chat` 点名的，合在一起去重，照模型名排；
//!   `listed` 照 `config`、`provider`、`catalog` 的先后写从哪几处列出来的。
//! - 模型的 `state`：写了 key、一个都没有值的是 `no_key`，别的是 `ok`（冷却随 8-9）。key 的 `state` 现在都是 `ok`。
//! - 这一家用不了（推不出驱动、地址，驱动还没有）：驱动、地址照手写的写，没写的是 `null`，带上 `problem` 那一句，没有模型。

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

use miyu_config::secret::Reference;
use miyu_models::facts::facts;
use miyu_models::matching::Found;
use miyu_models::provider::{self, Driver, NoModel};
use miyu_models::reference::{Place, Reference as ModelRef};
use miyu_models::settings::ProviderSettings;
use miyu_session::ModelData;

use super::Snapshot;

/// 编号 `id` 这一家。
pub(crate) fn provider(data: &ModelData, snapshot: &Snapshot, id: &str) -> Value {
    let values = snapshot.resolved.values();
    let settings = ProviderSettings::at(&values, &[id]);
    let keys: Vec<Value> = settings
        .keys
        .iter()
        .map(|reference| {
            json!({
                "ref": key_ref(reference),
                "set": snapshot.secret(reference).is_some(),
                "state": "ok",
            })
        })
        .collect();
    let no_key = !settings.keys.is_empty() && keys.iter().all(|key| key["set"] == false);
    data.with(
        |knowledge| match provider::provider(&values, knowledge, id) {
            Err(NoModel(problem)) => json!({
                "id": id,
                "driver": settings.driver,
                "base_url": settings.base_url,
                "keys": keys,
                "problem": problem,
                "models": [],
            }),
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
                        let state = if no_key { "no_key" } else { "ok" };
                        let mut entry = json!({
                            "model": model,
                            "ref": format!("{id}/{model}"),
                            "listed": places,
                            "facts": facts.json(&|layer| snapshot.file(layer)),
                            "state": state,
                        });
                        if let Found::Missing(missing) = matched {
                            entry["catalog_missing"] = json!(missing);
                        }
                        entry
                    })
                    .collect();
                let driver = match found.driver {
                    Driver::OpenAiChat => "openai-chat",
                };
                let mut entry = json!({
                    "id": id,
                    "driver": driver,
                    "base_url": found.base_url,
                    "keys": keys,
                    "models": models,
                });
                if let Some(recognized) = recognized {
                    entry["catalog"] =
                        json!({"provider": recognized.provider, "how": recognized.how.as_str()});
                }
                entry
            }
        },
    )
}

/// key 写成 `secret:<名字>`、`env:<变量>`。
fn key_ref(reference: &Reference) -> String {
    match reference {
        Reference::Secret(name) => format!("secret:{name}"),
        Reference::Env(name) => format!("env:{name}"),
    }
}

/// 配置里提到的这一家的模型：手写了资料的，`models.chat` 点名的。
fn written_models(values: &miyu_config::Values, id: &str) -> Vec<String> {
    let mut models = miyu_config::key::names(values.keys(), "providers.<id>.models.<model>", &[id]);
    let chat = provider::chat(values).and_then(|chat| ModelRef::parse_at(&chat, Place::Use).ok());
    if let Some(ModelRef::Model { provider, model }) = chat
        && provider == id
    {
        models.push(model);
    }
    models
}
