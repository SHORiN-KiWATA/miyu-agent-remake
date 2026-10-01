//! 选一家（`docs/blueprint/cli/setup.md`「怎么走」第 3 到 6 条，施工 8-11）：`provider.detect` 找到的、搜目录的、
//! `--provider` 写的。
//!
//! - 找到的：`keys` 在前、`local` 在后，用不了的不编号、灰字；头看得到、核心看不到的变量先说一段灰字。
//! - 搜目录：一次最多 [`PAGE`] 家，搜到的本机的一家不要 key，别的要贴。
//! - `--env` 写了的，要 key 的都照它（已经配好的、本机的不要）。

use serde_json::{Value, json};

use super::flow::{Chosen, Flow, Key};
use super::pick::{Row, Typed, numbered, typed};
use crate::shown::{say, write};

/// 搜目录一次列几家。
const PAGE: usize = 20;

/// `--provider` 找那一家时最多看几家：编号里有这一截的都在里面。
const ALL: usize = 1000;

/// 能选的一个，和它在表里的那一行。
struct Offer {
    chosen: Chosen,
    row: Row,
}

impl Flow<'_> {
    /// 从 `provider.detect` 的回应 `detected` 里选；都没有、选 `0` 的搜目录。
    pub(super) async fn choose(&mut self, detected: &Value) -> Result<Chosen, u8> {
        let language = self.plan.language;
        self.unseen(detected);
        let offers = self.offers(detected);
        if offers.is_empty() {
            say(self.err, language.found_nothing());
            return self.search().await;
        }
        say(self.err, language.found_heading());
        let rows: Vec<Row> = offers.iter().map(|offer| offer.row.clone()).collect();
        for line in numbered(&rows, Some(("0", language.none_of_these()))) {
            write(self.err, &line.paint(self.plan.gray));
        }
        let usable: Vec<&Offer> = offers.iter().filter(|offer| offer.row.usable).collect();
        loop {
            match typed(self.ask(language.pick_number()), usable.len()) {
                Typed::Picked(at) => return Ok(self.with_env(usable[at].chosen.clone())),
                Typed::Zero => return self.search().await,
                Typed::Empty | Typed::End => {
                    say(self.err, language.not_picked());
                    return Err(crate::exit::ERROR);
                }
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

    /// 找到的每一个写成能选的一行。
    fn offers(&self, detected: &Value) -> Vec<Offer> {
        let language = self.plan.language;
        let configured = |entry: &Value| entry["configured"].as_str().map(str::to_string);
        let mut offers: Vec<Offer> = list(&detected["keys"])
            .map(|key| {
                let env = text(&key["env"]);
                let mut place = language.found_key(&env);
                if let Some(id) = configured(key) {
                    place.push_str(&language.already_set_up(&id));
                }
                let usable = key["supported"] == json!(true);
                if !usable {
                    let why = language.unusable_why(key["driver"].as_str(), true);
                    place.push_str(&language.unusable(&why, true));
                }
                Offer {
                    chosen: Chosen {
                        id: text(&key["provider"]),
                        name: text(&key["name"]),
                        key: configured(key).map_or(Key::Env(env), Key::Configured),
                    },
                    row: Row {
                        cells: vec![text(&key["name"]), place],
                        usable,
                    },
                }
            })
            .collect();
        offers.extend(list(&detected["local"]).map(|service| {
            let models = service["models"].as_array().map_or(0, Vec::len);
            let mut place = language.found_local(&text(&service["base_url"]), models);
            if let Some(id) = configured(service) {
                place.push_str(&language.already_set_up(&id));
            }
            Offer {
                chosen: Chosen {
                    id: text(&service["provider"]),
                    name: text(&service["name"]),
                    key: configured(service).map_or(Key::Nothing, Key::Configured),
                },
                row: Row::usable(vec![text(&service["name"]), place]),
            }
        }));
        offers
    }

    /// 搜目录：问一截，列出来，选一个或者再搜。
    async fn search(&mut self) -> Result<Chosen, u8> {
        let language = self.plan.language;
        let not_picked = |flow: &mut Flow<'_>| {
            say(flow.err, language.not_picked());
            Err(crate::exit::ERROR)
        };
        let Some(mut query) = self.ask(language.search_for()) else {
            return not_picked(self);
        };
        loop {
            let query_now = query.trim().to_string();
            let mut params = json!({"limit": PAGE});
            if !query_now.is_empty() {
                params["query"] = json!(query_now);
            }
            let found = self.request("provider.catalog", params).await?;
            let providers: Vec<&Value> = list(&found["providers"]).collect();
            if providers.is_empty() {
                say(self.err, language.nothing_matches());
                match self.ask(language.search_for()) {
                    Some(next) => {
                        query = next;
                        continue;
                    }
                    None => return not_picked(self),
                }
            }
            let rows: Vec<Row> = providers
                .iter()
                .map(|entry| self.catalog_row(entry))
                .collect();
            for line in numbered(&rows, None) {
                write(self.err, &line.paint(self.plan.gray));
            }
            if providers.len() == PAGE {
                say(self.err, &language.only_first(PAGE));
            }
            let usable: Vec<&&Value> = providers
                .iter()
                .filter(|entry| entry["supported"] == json!(true))
                .collect();
            match typed(self.ask(language.pick_or_search()), usable.len()) {
                Typed::Picked(at) => return Ok(self.with_env(from_catalog(usable[at]))),
                Typed::Empty | Typed::End => return not_picked(self),
                Typed::Zero => query = "0".to_string(),
                Typed::Other(text) => query = text,
            }
        }
    }

    /// 搜到的一家写成一行：名字、编号，用不了的接原因。
    fn catalog_row(&self, entry: &Value) -> Row {
        let language = self.plan.language;
        let usable = entry["supported"] == json!(true);
        let note = match usable {
            true => String::new(),
            false => {
                let why =
                    language.unusable_why(entry["driver"].as_str(), !entry["base_url"].is_null());
                language.unusable(&why, false)
            }
        };
        Row {
            cells: vec![text(&entry["name"]), text(&entry["id"]), note],
            usable,
        }
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
