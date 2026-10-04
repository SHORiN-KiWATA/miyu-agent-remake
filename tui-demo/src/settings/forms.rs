//! 页内草稿和变更：普通格只发送修改项，基础价格整份保存；冲突预期取打开草稿时的个人层快照。

use super::data::{self, Model, Pool, Provider};
use serde_json::{Value, json};

/// 一格如何编辑。
#[derive(Clone)]
pub enum Kind {
    /// 核心按配置类型解析人敲的文字；空的恢复默认。
    Input,
    /// 选项，首项为空表示恢复默认。
    Choice(Vec<String>),
    /// 多选输入能力。
    Inputs,
    /// 不改标识，以免已有引用失效。
    ReadOnly,
    /// 环境变量、已存密钥引用，或者临时明文。
    Key,
}

/// 一格的草稿，无 Debug，避免密钥泄露。
pub struct Field {
    /// 文案资源键。
    pub label: String,
    /// 配置键；标识格不写配置。
    pub key: String,
    /// 当前草稿。
    pub value: String,
    /// 编辑前的值。
    pub original: String,
    /// 编辑类型。
    pub kind: Kind,
    /// 核心给的来源（不含秘密）。
    pub source: String,
}

impl Field {
    /// 已经编辑过。
    pub fn changed(&self) -> bool {
        self.value != self.original
    }
    /// 密钥明文在列表里只显示占位。
    pub fn shown(&self) -> &str {
        if matches!(self.kind, Kind::Key)
            && !self.value.is_empty()
            && !self.value.starts_with("$env:")
            && !self.value.starts_with("$secret:")
        {
            "••••••••"
        } else {
            &self.value
        }
    }
}

/// 草稿是哪种资料。
pub enum Entity {
    /// 新供应商或已有供应商。
    Provider { new: bool },
    /// 模型手写覆盖。
    Model,
    /// 池，有序成员另存。
    Pool {
        new: bool,
        members: Vec<String>,
        before: Vec<String>,
    },
}

/// 编辑页：不从后台刷新覆盖草稿。
pub struct Form {
    /// 页头对象名。
    pub title: String,
    /// 每一格。
    pub fields: Vec<Field>,
    /// 原配置完整来源；保存时只用这里的个人层 expect。
    pub config: Value,
    /// 编辑类型。
    pub entity: Entity,
}

fn string(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn field(config: &Value, label: &str, key: String, fallback: Value, kind: Kind) -> Field {
    let item = &config["items"][&key];
    let effective = if item["value"].is_null() {
        &fallback
    } else {
        &item["value"]
    };
    let value = string(effective);
    let source = item["origin"]["layer"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    Field {
        label: label.to_string(),
        key,
        original: value.clone(),
        value,
        kind,
        source,
    }
}

fn id(value: &str, new: bool) -> Field {
    Field {
        label: "id".into(),
        key: String::new(),
        value: value.into(),
        original: value.into(),
        source: String::new(),
        kind: if new { Kind::Input } else { Kind::ReadOnly },
    }
}

/// 供应商编辑。密钥当前只显示引用；不触碰已有的多个 key，除非用户编辑这一格。
pub fn provider(config: &Value, p: Option<&Provider>) -> Form {
    let name = p.map_or("", |p| p.id.as_str());
    let base = format!("providers.{}", data::segment(name));
    let raw = data::value(config, &format!("{base}.keys"));
    let keys = raw.as_array().filter(|v| v.len() == 1).map(|v| &v[0]);
    let key = keys.map_or_else(
        || {
            if raw.as_array().is_some_and(|v| !v.is_empty()) {
                raw.to_string()
            } else {
                String::new()
            }
        },
        |v| {
            if let Some(name) = v["env"].as_str() {
                format!("$env:{name}")
            } else if let Some(name) = v["secret"].as_str() {
                format!("$secret:{name}")
            } else {
                String::new()
            }
        },
    );
    let mut fields = vec![
        id(name, p.is_none()),
        field(
            config,
            "base_url",
            format!("{base}.base_url"),
            Value::Null,
            Kind::Input,
        ),
        field(
            config,
            "driver",
            format!("{base}.driver"),
            Value::Null,
            Kind::Choice(super::options::get().drivers.clone()),
        ),
        Field {
            label: "key".into(),
            key: format!("{base}.keys"),
            original: key.clone(),
            value: key,
            kind: Kind::Key,
            source: String::new(),
        },
    ];
    // Base URL 的环境引用也保持可读可改，不把对象转成一次普通网址。
    let url = data::value(config, &fields[1].key);
    if let Some(env) = url["env"].as_str() {
        fields[1].value = format!("$env:{env}");
        fields[1].original = fields[1].value.clone();
    }
    Form {
        title: p.map_or_else(String::new, |p| p.name.clone()),
        fields,
        config: config.clone(),
        entity: Entity::Provider { new: p.is_none() },
    }
}

/// 模型资料按核心最终值显示，保存只写个人覆盖。
pub fn model(config: &Value, p: &Provider, m: &Model) -> Form {
    let base = m.facts["effort"]["key"]
        .as_str()
        .and_then(|k| k.strip_suffix(".effort"))
        .map(str::to_string)
        .unwrap_or_else(|| {
            format!(
                "providers.{}.models.{}",
                data::segment(&p.id),
                data::segment(&m.id)
            )
        });
    let mut levels = vec![String::new()];
    levels.extend(
        m.facts["reasoning"]["value"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str().map(str::to_string)),
    );
    let mut fields = vec![
        field(
            config,
            "inputs",
            format!("{base}.inputs"),
            m.facts["inputs"]["value"].clone(),
            Kind::Inputs,
        ),
        field(
            config,
            "window",
            format!("{base}.window"),
            m.facts["window"]["value"].clone(),
            Kind::Input,
        ),
        field(
            config,
            "effort",
            format!("{base}.effort"),
            m.facts["effort"]["value"].clone(),
            Kind::Choice(levels),
        ),
    ];
    for key in ["input", "output", "cache_read", "cache_write", "currency"] {
        fields.push(field(
            config,
            key,
            format!("{base}.price.{key}"),
            if key == "currency" && m.facts["price"]["value"][key].is_null() {
                json!("USD")
            } else {
                m.facts["price"]["value"][key].clone()
            },
            Kind::Input,
        ));
    }
    fields.push(field(
        config,
        "multiplier",
        format!("{base}.price_multiplier"),
        m.facts["multiplier"]["value"].clone(),
        Kind::Input,
    ));
    for f in &mut fields {
        if f.source.is_empty() {
            let fact = if f.key.contains(".price.") {
                "price"
            } else if f.label == "multiplier" {
                "multiplier"
            } else {
                &f.label
            };
            f.source = m.facts[fact]["from"]
                .as_str()
                .unwrap_or_default()
                .to_string();
        }
    }
    Form {
        title: m.reference.clone(),
        fields,
        config: config.clone(),
        entity: Entity::Model,
    }
}

/// 池只有标识、调用方式和成员。
pub fn pool(config: &Value, p: Option<&Pool>) -> Form {
    let name = p.map_or("", |p| p.name.as_str());
    let base = format!("pools.{}", data::segment(name));
    let members = p.map_or_else(Vec::new, |p| p.models.clone());
    Form {
        title: name.into(),
        config: config.clone(),
        fields: vec![
            id(name, p.is_none()),
            field(
                config,
                "strategy",
                format!("{base}.strategy"),
                p.map_or(Value::Null, |p| json!(p.strategy)),
                Kind::Choice(super::options::get().strategies.clone()),
            ),
        ],
        entity: Entity::Pool {
            new: p.is_none(),
            before: members.clone(),
            members,
        },
    }
}

impl Form {
    /// 保存变更。基础价格编辑会保存整份显示值；其余只写修改项，不夹带明文密钥。
    pub fn changes(&self) -> Vec<Value> {
        let mut changes = Vec::new();
        let identifier = self.fields.first().map_or("", |f| f.value.as_str());
        let new = matches!(
            self.entity,
            Entity::Provider { new: true } | Entity::Pool { new: true, .. }
        );
        // 核心把手写价格作为整份资料；只写一项会丢失其他目录价。
        // 空项明确取消个人覆盖，不把未知价补成 0；每项仍带自己的 expect。
        let price_field = |f: &Field| {
            matches!(
                f.label.as_str(),
                "input" | "output" | "cache_read" | "cache_write" | "currency"
            )
        };
        let complete_price = matches!(self.entity, Entity::Model)
            && self.fields.iter().any(|f| price_field(f) && f.changed());
        for f in &self.fields {
            if f.key.is_empty() || (!new && !f.changed() && !(complete_price && price_field(f))) {
                continue;
            }
            let key = if new {
                let suffix = f
                    .key
                    .split_once(".\"\".")
                    .map(|(_, s)| s)
                    .unwrap_or_default();
                let group = if matches!(self.entity, Entity::Provider { .. }) {
                    "providers"
                } else {
                    "pools"
                };
                format!("{group}.{}.{suffix}", data::segment(identifier))
            } else {
                f.key.clone()
            };
            let change = if matches!(f.kind, Kind::Key) {
                let value = if let Some(env) = f.value.strip_prefix("$env:") {
                    json!([{"env":env}])
                } else if let Some(secret) = f.value.strip_prefix("$secret:") {
                    json!([{"secret":secret}])
                } else if f.value.trim().is_empty() {
                    json!([])
                } else {
                    continue;
                }; // 明文必须先通过 secret.set，不能塞进配置。
                json!({"key":key,"value":value})
            } else if f.value.trim().is_empty() {
                json!({"key":key,"unset":true})
            } else if matches!(f.kind, Kind::Inputs) {
                let Ok(value) = serde_json::from_str::<Vec<String>>(&f.value) else {
                    continue;
                };
                json!({"key":key,"value":value})
            } else if let Some(env) = f
                .value
                .strip_prefix("$env:")
                .filter(|_| f.label == "base_url")
            {
                json!({"key":key,"value":{"env":env}})
            } else {
                json!({"key":key,"input":f.value})
            };
            changes.push(data::guarded(&self.config, change));
        }
        if let Entity::Pool {
            new,
            members,
            before,
        } = &self.entity
            && (*new || members != before)
        {
            let key = format!("pools.{}.models", data::segment(identifier));
            changes.push(data::guarded(
                &self.config,
                json!({"key":key,"value":members}),
            ));
        }
        changes
    }
}

#[cfg(test)]
mod tests;
