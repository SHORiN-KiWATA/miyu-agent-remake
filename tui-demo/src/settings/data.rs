//! 配置页读来的数据（蓝图「配置页」第 23、25 条）：`model.list` 的供应商、模型、池、默认用途，`config.get`（`all`）里
//! 个人层、系统层各写了哪几项，`secret.list` 的名字。只读；改的东西在草稿里（`draft.rs`）。

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use super::keys;

/// 读来的一份。
#[derive(Debug, Clone, Default)]
pub struct Data {
    /// 配好的供应商，照核心交回的先后。
    pub providers: Vec<Provider>,
    /// 模型池，照核心交回的先后。
    pub pools: Vec<Pool>,
    /// 默认的文本模型（`uses.chat`）。
    pub chat: Option<String>,
    /// 默认的视觉模型（`uses.vision`）。
    pub vision: Option<String>,
    /// 个人层写着的每一项：键 → 值（存的时候做 `expect`）。
    pub personal: BTreeMap<String, Value>,
    /// 系统层写了的键：删供应商、池时看写没写在这里。
    pub system: BTreeSet<String>,
    /// 密钥库里已有的名字：新存的 key 不撞它们。
    pub secrets: BTreeSet<String>,
}

/// 一家供应商。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Provider {
    /// 编号。
    pub id: String,
    /// 显示名（核心 8-21 的 `name.value`）；没有的界面写编号。
    pub name: Option<String>,
    /// 地址：写死的是字，环境变量引用的是 `{"env": …}`，没写的是 `null`。
    pub base_url: Value,
    /// 写了的驱动；没写的是 `None`（核心照目录、地址推）。
    pub driver: Option<String>,
    /// 每个 key 的引用（`secret:<名字>`、`env:<变量>`）和设没设。
    pub keys: Vec<(String, bool)>,
    /// 模型，照核心交回的先后。
    pub models: Vec<Model>,
    /// 用不了的原因（`problem`）。
    pub problem: Option<String>,
}

/// 一个模型。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Model {
    /// 模型名（照供应商那边的叫法，可能带 `组织/`）。
    pub name: String,
    /// 引用：`供应商/模型`。
    pub reference: String,
    /// 从哪几处列出来的（`config`、`provider`、`catalog`）。
    pub listed: Vec<String>,
    /// 资料：每一格 `{value, from, …}`，原样留着。
    pub facts: Value,
    /// 这个模型在配置里那张表的键（`providers.<id>.models."<模型>"`）。
    pub table: String,
}

/// 一个模型池。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Pool {
    /// 名字。
    pub name: String,
    /// 生效的分法：`pin`、`rotate`。
    pub strategy: Option<String>,
    /// 成员，照配置写的先后（认不出的也在）。
    pub members: Vec<String>,
}

impl Data {
    /// 照 `model.list` 的回应读供应商、模型、池、默认用途。
    pub fn read_models(&mut self, list: &Value) {
        self.providers = list["providers"]
            .as_array()
            .map(|all| all.iter().map(provider).collect())
            .unwrap_or_default();
        self.pools = list["pools"]
            .as_array()
            .map(|all| all.iter().map(pool).collect())
            .unwrap_or_default();
        self.chat = list["uses"]["chat"].as_str().map(str::to_string);
        self.vision = list["uses"]["vision"].as_str().map(str::to_string);
    }

    /// 照 `config.get`（`all: true`）的回应记下个人层写着的值、系统层写了的键。
    pub fn read_config(&mut self, got: &Value) {
        self.personal.clear();
        self.system.clear();
        let Some(items) = got["items"].as_object() else {
            return;
        };
        for (key, item) in items {
            for layer in item["layers"].as_array().into_iter().flatten() {
                match layer["origin"]["layer"].as_str() {
                    Some("personal") => {
                        self.personal.insert(key.clone(), layer["value"].clone());
                    }
                    Some("system") => {
                        self.system.insert(key.clone());
                    }
                    _ => {}
                }
            }
        }
    }

    /// 照 `secret.list` 的回应记下已有的名字。
    pub fn read_secrets(&mut self, got: &Value) {
        self.secrets = got["secrets"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|s| s["name"].as_str().map(str::to_string))
            .collect();
    }

    /// 照编号找供应商。
    pub fn provider(&self, id: &str) -> Option<&Provider> {
        self.providers.iter().find(|p| p.id == id)
    }

    /// 照引用找模型（`供应商/模型`）。
    pub fn model(&self, reference: &str) -> Option<(&Provider, &Model)> {
        self.providers.iter().find_map(|p| {
            p.models
                .iter()
                .find(|m| m.reference == reference)
                .map(|m| (p, m))
        })
    }

    /// 照名字找池。
    pub fn pool(&self, name: &str) -> Option<&Pool> {
        self.pools.iter().find(|p| p.name == name)
    }

    /// 这个前缀下面，系统层写了东西：这里删不掉。
    pub fn in_system(&self, prefix: &str) -> bool {
        self.system.iter().any(|k| keys::under(k, prefix))
    }

    /// 这个前缀下面个人层写着的键。
    pub fn personal_under(&self, prefix: &str) -> Vec<String> {
        self.personal
            .keys()
            .filter(|k| keys::under(k, prefix))
            .cloned()
            .collect()
    }
}

impl Provider {
    /// 界面上写的名字：显示名，没有的写编号。
    pub fn shown(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.id)
    }
}

impl Model {
    /// 一格资料的值。
    pub fn fact(&self, name: &str) -> &Value {
        &self.facts[name]["value"]
    }

    /// 一格资料从哪来：`catalog`、`provider`、`default`，配置里写的是 `config` 加层（`config:personal`）。
    pub fn from(&self, name: &str) -> String {
        let fact = &self.facts[name];
        match (fact["from"].as_str(), fact["layer"].as_str()) {
            (Some("config"), Some(layer)) => format!("config:{layer}"),
            (Some(from), _) => from.to_string(),
            (None, _) => "default".to_string(),
        }
    }

    /// 上下文窗口。
    pub fn window(&self) -> Option<u64> {
        self.fact("window").as_u64()
    }

    /// 能收哪几种（`text`、`image`、`pdf`）。
    pub fn inputs(&self) -> Vec<String> {
        strings(self.fact("inputs"))
    }

    /// 能收图。
    pub fn sees(&self) -> bool {
        self.inputs().iter().any(|i| i == "image")
    }

    /// 只在配置里写着（`n` 加的）：只有这种能删。
    pub fn custom(&self) -> bool {
        self.listed.iter().all(|l| l == "config") && !self.listed.is_empty()
    }
}

fn provider(p: &Value) -> Provider {
    let id = text(&p["id"]);
    let models = p["models"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|m| model(&id, m))
        .collect();
    Provider {
        name: p["name"]["value"]
            .as_str()
            .or(p["name"].as_str())
            .filter(|n| !n.is_empty())
            .map(str::to_string),
        base_url: p["base_url"].clone(),
        driver: p["driver"].as_str().map(str::to_string),
        keys: p["keys"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|k| (text(&k["ref"]), k["set"].as_bool().unwrap_or(false)))
            .collect(),
        models,
        problem: p["problem"].as_str().map(str::to_string),
        id,
    }
}

fn model(provider: &str, m: &Value) -> Model {
    let name = text(&m["model"]);
    // 核心给了 `effort` 的完整键名就照它（引号怎么加以它为准），没给的自己拼。
    let table = m["facts"]["effort"]["key"]
        .as_str()
        .and_then(|k| k.strip_suffix(".effort"))
        .map_or_else(|| keys::model_table(provider, &name), str::to_string);
    Model {
        reference: m["ref"]
            .as_str()
            .map_or_else(|| format!("{provider}/{name}"), str::to_string),
        listed: strings(&m["listed"]),
        facts: m["facts"].clone(),
        table,
        name,
    }
}

fn pool(p: &Value) -> Pool {
    Pool {
        name: text(&p["name"]),
        strategy: p["strategy"].as_str().map(str::to_string),
        members: strings(&p["models"]),
    }
}

fn text(v: &Value) -> String {
    v.as_str().unwrap_or_default().to_string()
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| s.as_str().map(str::to_string))
        .collect()
}

#[cfg(test)]
mod tests;
