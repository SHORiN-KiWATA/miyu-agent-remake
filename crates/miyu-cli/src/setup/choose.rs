//! 选一家（`docs/blueprint/cli/setup.md`「怎么走」第 3 到 6 条，施工 8-11、8-11 再补）：常用的几家、本机跑着的服务、自定义，
//! 或者 `--provider` 写的。
//!
//! - 常用的几家照 `provider.catalog {"featured": true}` 的先后（2026-10-08 项目主人定）：配好了的标「已配好」，找到了 key 的
//!   标「已找到 key」，用不了的不编号、灰字；本机跑着的服务标「本机」；最后一行「自定义」（`custom.rs`）。
//! - 头看得到、核心看不到的变量先说一段灰字。
//! - `--env` 写了的，要 key 的都照它（已经配好的、本机的不要）。

use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::flow::{Chosen, Flow, Key};
use super::pick::{Row, Typed, numbered, typed};
use crate::shown::{say, write};

/// `--provider` 找那一家时最多看几家：编号里有这一截的都在里面。
const ALL: usize = 1000;

/// 表里的一行：选了是哪一家（「自定义」的没有，接着问），和它那一行。
struct Offer {
    chosen: Option<Chosen>,
    row: Row,
}

impl Flow<'_> {
    /// 列常用的几家、本机跑着的服务、自定义，选一个。`detected` 是 `provider.detect` 的回应。
    pub(super) async fn choose(&mut self, detected: &Value) -> Result<Chosen, u8> {
        let language = self.plan.language;
        self.unseen(detected);
        let featured = self
            .request("provider.catalog", json!({"featured": true}))
            .await?;
        let configured = self.configured().await?;
        let offers = self.offers(detected, &featured, &configured);
        say(self.err, language.pick_provider());
        let rows: Vec<Row> = offers.iter().map(|offer| offer.row.clone()).collect();
        for line in numbered(&rows, None) {
            write(self.err, &line.paint(self.plan.gray));
        }
        let usable: Vec<&Offer> = offers.iter().filter(|offer| offer.row.usable).collect();
        loop {
            match typed(self.ask(language.pick_number()), usable.len()) {
                Typed::Picked(at) => {
                    return match &usable[at].chosen {
                        Some(chosen) => Ok(self.with_env(chosen.clone())),
                        None => self.custom(),
                    };
                }
                Typed::Empty | Typed::End => {
                    say(self.err, language.not_picked());
                    return Err(crate::exit::ERROR);
                }
                Typed::Zero => say(self.err, &language.not_a_number("0")),
                Typed::Other(text) => say(self.err, &language.not_a_number(&text)),
            }
        }
    }

    /// 头这边设了、找了、核心的 `keys` 里没有的变量：说一段灰字。
    fn unseen(&mut self, detected: &Value) {
        let seen: Vec<&str> = list(&detected["keys"])
            .filter_map(|key| key["env"].as_str())
            .collect();
        let missing: Vec<&str> = list(&detected["looked_for"])
            .filter_map(Value::as_str)
            .filter(|name| !seen.contains(name) && self.plan.here.has(name))
            .collect();
        if !missing.is_empty() {
            self.gray(self.plan.language.core_cannot_see(&missing));
        }
    }

    /// 配置里已经有的几家：目录里的编号 → 配置里的编号（写了 `catalog` 的照它，没写的就是配置里的编号）。照 `config.get`
    /// 的最终值，配置里的编号照字节排、先占的算。
    async fn configured(&mut self) -> Result<BTreeMap<String, String>, u8> {
        let got = self.request("config.get", json!({})).await?;
        let items = got["items"].as_object().cloned().unwrap_or_default();
        let mut configured = BTreeMap::new();
        for key in items.keys() {
            let Some(id) = key
                .strip_prefix("providers.")
                .and_then(|rest| rest.split_once('.'))
                .map(|(id, _)| id)
            else {
                continue;
            };
            let catalog = items
                .get(&format!("providers.{id}.catalog"))
                .and_then(|item| item["value"].as_str())
                .unwrap_or(id);
            configured
                .entry(catalog.to_string())
                .or_insert_with(|| id.to_string());
        }
        Ok(configured)
    }

    /// 表里的每一行：常用的几家、本机跑着的服务，最后「自定义」。
    fn offers(
        &self,
        detected: &Value,
        featured: &Value,
        configured: &BTreeMap<String, String>,
    ) -> Vec<Offer> {
        let language = self.plan.language;
        let mut offers: Vec<Offer> = list(&featured["providers"])
            .map(|entry| {
                let id = text(&entry["id"]);
                let found = list(&detected["keys"])
                    .find(|key| key["provider"] == json!(id) && key["supported"] == json!(true));
                let (key, mark) = match (configured.get(&id), found) {
                    (Some(set), _) => (Key::Configured(set.clone()), language.set_up_mark()),
                    (None, Some(found)) => (Key::Env(text(&found["env"])), language.key_found()),
                    (None, None) => (from_catalog(entry).key, ""),
                };
                let usable = entry["supported"] == json!(true);
                let note = match usable {
                    true => mark.to_string(),
                    false => {
                        let why = language
                            .unusable_why(entry["driver"].as_str(), !entry["base_url"].is_null());
                        language.unusable(&why, false)
                    }
                };
                Offer {
                    chosen: Some(Chosen {
                        id,
                        name: text(&entry["name"]),
                        key,
                        custom: None,
                    }),
                    row: Row {
                        cells: vec![text(&entry["name"]), note],
                        usable,
                    },
                }
            })
            .collect();
        offers.extend(list(&detected["local"]).map(|service| {
            let mut note = language.local_mark(&text(&service["base_url"]));
            let key = match service["configured"].as_str() {
                Some(id) => {
                    note.push_str(&format!("，{}", language.set_up_mark()));
                    Key::Configured(id.to_string())
                }
                None => Key::Nothing,
            };
            Offer {
                chosen: Some(Chosen {
                    id: text(&service["provider"]),
                    name: text(&service["name"]),
                    key,
                    custom: None,
                }),
                row: Row::usable(vec![text(&service["name"]), note]),
            }
        }));
        offers.push(Offer {
            chosen: None,
            row: Row::usable(vec![language.custom().to_string(), String::new()]),
        });
        offers
    }

    /// `--provider`：目录、档案里编号一模一样、能用的那一家；找到了它的 key 的照那个变量用。
    pub(super) async fn chosen_by_param(
        &mut self,
        id: &str,
        detected: &Value,
    ) -> Result<Chosen, u8> {
        let found = self
            .request("provider.catalog", json!({"query": id, "limit": ALL}))
            .await?;
        let Some(entry) = list(&found["providers"])
            .find(|entry| entry["id"] == json!(id) && entry["supported"] == json!(true))
        else {
            say(self.err, &self.plan.language.no_usable_provider(id));
            return Err(super::MISUSE);
        };
        let mut chosen = from_catalog(entry);
        let key = list(&detected["keys"])
            .find(|key| key["provider"] == json!(id) && key["supported"] == json!(true));
        if let (Some(key), Key::Paste) = (key, &chosen.key) {
            chosen.key = Key::Env(text(&key["env"]));
        }
        Ok(self.with_env(chosen))
    }

    /// 写了 `--env` 的：要 key 的照它（已经配好的、本机的不要 key，不动）。
    fn with_env(&self, mut chosen: Chosen) -> Chosen {
        if let (Some(name), Key::Paste | Key::Env(_)) = (&self.plan.setup.env, &chosen.key) {
            chosen.key = Key::Env(name.clone());
        }
        chosen
    }
}

/// 目录里的一家：本机的不要 key，别的要贴。
fn from_catalog(entry: &Value) -> Chosen {
    Chosen {
        id: text(&entry["id"]),
        name: text(&entry["name"]),
        key: match entry["local"] == json!(true) {
            true => Key::Nothing,
            false => Key::Paste,
        },
        custom: None,
    }
}

/// 一个数组里的每一个；不是数组的什么都没有。
fn list(value: &Value) -> impl Iterator<Item = &Value> {
    value.as_array().into_iter().flatten()
}

/// 一格字；不是字的是空的。
fn text(value: &Value) -> String {
    value.as_str().unwrap_or_default().to_string()
}
