//! 草稿（蓝图「配置页」第 16、24 条）：悬浮窗「确定」记进这里，`s` 一起存。草稿就是要发的那条 `config.set`：键 → 改成什么
//! 或删掉；另记要存进密钥库的明文 key（照供应商记，存的时候才起名字）。平常写个人设置；只有系统配置这一层的项（运行日志
//! 级别这类，第 33 条）当场存，那一次写系统配置。

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

use super::data::Data;
use super::keys;

/// 一项改成什么。
#[derive(Debug, Clone, PartialEq)]
pub enum Change {
    /// 写成这个值。
    Set(Value),
    /// 从个人层删掉，回到下面一层的。
    Unset,
}

/// 没存的改动。
#[derive(Debug, Default)]
pub struct Draft {
    changes: BTreeMap<String, Change>,
    /// 供应商编号 → 新贴的 key（明文，只在这里和发 `secret.set` 那一刻）。
    secrets: BTreeMap<String, String>,
    /// 新加的供应商（编号），照加的先后。
    pub new_providers: Vec<String>,
    /// 新加的模型（供应商编号、模型名），照加的先后。
    pub new_models: Vec<(String, String)>,
    /// 新建的池，照建的先后。
    pub new_pools: Vec<String>,
    /// 这一份写系统配置（不是个人设置）：改只有系统配置这一层的项时设上，存完、扔掉时回到个人设置。
    pub system: bool,
}

impl Draft {
    /// 写到的那一层现在写着的：个人设置，或者系统配置（[`Draft::system`]）。
    fn written<'a>(&self, data: &'a Data) -> &'a BTreeMap<String, Value> {
        if self.system {
            &data.system
        } else {
            &data.personal
        }
    }

    /// 改成 `value`；和那一层现在写的一样的不算改动。
    pub fn set(&mut self, key: &str, value: Value, data: &Data) {
        if self.written(data).get(key) == Some(&value) {
            self.changes.remove(key);
        } else {
            self.changes.insert(key.to_string(), Change::Set(value));
        }
    }

    /// 从那一层删掉；本来就没写的不算改动。
    pub fn unset(&mut self, key: &str, data: &Data) {
        if self.written(data).contains_key(key) {
            self.changes.insert(key.to_string(), Change::Unset);
        } else {
            self.changes.remove(key);
        }
    }

    /// 删掉个人层里这张表下面写的每一项（删供应商、模型、池），连同草稿里新加的。
    pub fn unset_all(&mut self, prefix: &str, data: &Data) {
        self.changes.retain(|k, _| !keys::under(k, prefix));
        for key in data.personal_under(prefix) {
            self.changes.insert(key, Change::Unset);
        }
        if let Some(id) = prefix
            .strip_prefix("providers.")
            .filter(|r| !r.contains('.'))
        {
            self.secrets.remove(id);
            self.new_providers.retain(|p| p != id);
            self.new_models.retain(|(p, _)| p != id);
        }
        self.new_models
            .retain(|(p, m)| keys::model_table(p, m) != prefix);
        self.new_pools.retain(|p| keys::pool(p) != prefix);
    }

    /// 新贴了一家供应商的 key：存的时候先 `secret.set`，再把引用写进 `key`（一家一个，核心 8-25）。
    pub fn set_secret(&mut self, provider: &str, value: String) {
        self.changes
            .remove(&format!("{}.key", keys::provider(provider)));
        self.secrets.insert(provider.to_string(), value);
    }

    /// 草稿里这一项是什么：写了的值，删了的 `None`，没改的照那一层。
    pub fn value<'a>(&'a self, key: &str, data: &'a Data) -> Option<&'a Value> {
        match self.changes.get(key) {
            Some(Change::Set(v)) => Some(v),
            Some(Change::Unset) => None,
            None => self.written(data).get(key),
        }
    }

    /// 草稿里改过这一项。
    pub fn changed(&self, key: &str) -> bool {
        self.changes.contains_key(key)
    }

    /// 草稿把这张表在个人层写的全删了（个人层原来写了东西的才算）。
    pub fn removes(&self, prefix: &str, data: &Data) -> bool {
        let written = data.personal_under(prefix);
        !written.is_empty()
            && written
                .iter()
                .all(|k| self.changes.get(k) == Some(&Change::Unset))
            && !self
                .changes
                .iter()
                .any(|(k, c)| keys::under(k, prefix) && matches!(c, Change::Set(_)))
    }

    /// 什么都没改。
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
            && self.secrets.is_empty()
            && self.new_providers.is_empty()
            && self.new_pools.is_empty()
    }

    /// 全扔掉。
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// 草稿里写成值的每一项（拼出界面上看的那一份用）。
    pub fn sets(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.changes.iter().filter_map(|(k, c)| match c {
            Change::Set(v) => Some((k.as_str(), v)),
            Change::Unset => None,
        })
    }

    /// 这一家贴了新的 key、还没存。
    pub fn has_secret(&self, provider: &str) -> bool {
        self.secrets.contains_key(provider)
    }

    /// 存下以后要重新取模型列表的供应商：新加的、换了地址、接口、key 的；删掉的不算。
    pub fn reconnects(&self, data: &Data) -> Vec<String> {
        let mut ids: Vec<String> = self.new_providers.clone();
        ids.extend(self.secrets.keys().cloned());
        for (key, change) in &self.changes {
            let parts = split(key);
            if matches!(change, Change::Set(_))
                && parts.len() == 3
                && parts[0] == "providers"
                && matches!(parts[2].as_str(), "base_url" | "driver" | "key")
            {
                ids.push(parts[1].trim_matches('"').to_string());
            }
        }
        ids.sort();
        ids.dedup();
        ids.retain(|id| !self.removes(&keys::provider(id), data));
        ids
    }

    /// 要存进密钥库的：供应商、起好的名字（不撞已有的、不互相撞）、明文。
    pub fn secrets(&self, data: &Data) -> Vec<(String, String, String)> {
        let mut taken = data.secrets.clone();
        self.secrets
            .iter()
            .map(|(provider, value)| {
                let name = secret_name(provider, &taken);
                taken.insert(name.clone());
                (provider.clone(), name, value.clone())
            })
            .collect()
    }

    /// 要发的 `config.set` 参数：个人层（或系统配置），每项带 `expect`（那一层原来写的什么）；`named` 是存好了的 key
    /// （供应商、名字），引用写进这一家的 `key`。
    pub fn request(&self, data: &Data, named: &[(String, String)]) -> Value {
        let mut all = self.changes.clone();
        for (provider, name) in named {
            let key = format!("{}.key", keys::provider(provider));
            all.insert(key, Change::Set(json!({"secret": name})));
        }
        let changes: Vec<Value> = all
            .iter()
            .map(|(key, change)| {
                let expect = self
                    .written(data)
                    .get(key)
                    .map_or_else(|| json!({}), |v| json!({"value": v}));
                match change {
                    Change::Set(value) => json!({"key": key, "value": value, "expect": expect}),
                    Change::Unset => json!({"key": key, "unset": true, "expect": expect}),
                }
            })
            .collect();
        let layer = if self.system { "system" } else { "personal" };
        json!({"layer": layer, "changes": changes})
    }
}

/// 照 `.` 分段，引号里的点不算。
fn split(key: &str) -> Vec<String> {
    let mut parts = vec![String::new()];
    let mut quoted = false;
    let mut escaped = false;
    for c in key.chars() {
        let last = parts.last_mut().expect("至少一段");
        match c {
            _ if escaped => {
                last.push(c);
                escaped = false;
            }
            '\\' if quoted => {
                last.push(c);
                escaped = true;
            }
            '"' => {
                last.push(c);
                quoted = !quoted;
            }
            '.' if !quoted => parts.push(String::new()),
            _ => last.push(c),
        }
    }
    parts
}

/// 密钥的名字：`<编号>-key`，撞了接 `-2`、`-3`；名字最长 64 个字符（`config.md`「名字的写法」）。
fn secret_name(provider: &str, taken: &BTreeSet<String>) -> String {
    let base: String = format!("{provider}-key").chars().take(56).collect();
    if !taken.contains(&base) {
        return base;
    }
    (2..)
        .map(|n| format!("{base}-{n}"))
        .find(|name| !taken.contains(name))
        .expect("总有一个没用过的")
}

#[cfg(test)]
mod tests;
