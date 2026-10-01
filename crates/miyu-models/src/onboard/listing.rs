//! 目录和档案里的每一家（`docs/blueprint/models.md`「协议」`provider.catalog`、「怎么走」第七条第 1 到 3 条，施工 8-11）。
//!
//! - 一家的名字照档案的、再是目录的、都没有的是编号；驱动照档案的、再是目录的 `npm` 照 `[npm]` 换的；地址照档案的、再是
//!   目录的 `api`。和配好的一家推驱动、地址的先后一样（[`crate::provider`] 第 1 条），只是这里没有手写的。
//! - 驱动是现在有的（[`Driver::parse`]）、有地址的才 `supported`；地址在本机的是 `local`（[`on_this_machine`]）。
//! - 找哪些环境变量：目录里 `env` 只有一个名字的就是它的 key；几个名字的（Azure 那类要另给资源名的）不找。

use serde_json::{Value, json};

use crate::knowledge::Knowledge;
use crate::provider::{Driver, on_this_machine};

/// 目录、档案里的一家。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    /// 编号：目录里的、档案里的。
    pub id: String,
    /// 给人看的名字。
    pub name: String,
    /// 驱动的写法；认不出的没有。
    pub driver: Option<String>,
    /// 地址；没有的没有。
    pub base_url: Option<String>,
    /// 目录里找 key 的环境变量，原样。
    pub env: Vec<String>,
    /// 文档的网址。
    pub doc: Option<String>,
    /// 目录里有几个模型。
    pub models: usize,
    /// 驱动是现在有的、有地址：选得了。
    pub supported: bool,
    /// 地址在本机：不要 key。
    pub local: bool,
}

impl Listed {
    /// `provider.catalog` 回应里的一家。
    pub fn json(&self) -> Value {
        json!({
            "id": self.id,
            "name": self.name,
            "driver": self.driver,
            "base_url": self.base_url,
            "env": self.env,
            "doc": self.doc,
            "models": self.models,
            "supported": self.supported,
            "local": self.local,
        })
    }
}

/// 目录和档案里的每一家，照编号排（照字节）：两边都有的合成一家，档案的压过目录的。
pub fn listed(knowledge: &Knowledge<'_>) -> Vec<Listed> {
    let catalog = knowledge.catalog.map(|loaded| &loaded.catalog);
    let mut ids: Vec<&str> = catalog
        .into_iter()
        .flat_map(|catalog| catalog.providers().map(|entry| entry.id.as_str()))
        .chain(knowledge.profiles.providers.keys().map(String::as_str))
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids.into_iter()
        .map(|id| {
            let entry = catalog.and_then(|catalog| catalog.provider(id));
            let profile = knowledge.profiles.providers.get(id);
            let driver = profile
                .and_then(|profile| profile.driver.clone())
                .or_else(|| {
                    let npm = entry?.npm.as_ref()?;
                    knowledge.profiles.npm.get(npm).cloned()
                });
            let base_url = profile
                .and_then(|profile| profile.base_url.clone())
                .or_else(|| entry?.api.clone());
            let supported =
                driver.as_deref().and_then(Driver::parse).is_some() && base_url.is_some();
            Listed {
                id: id.to_string(),
                name: profile
                    .and_then(|profile| profile.name.clone())
                    .or_else(|| entry?.name.clone())
                    .unwrap_or_else(|| id.to_string()),
                local: base_url.as_deref().is_some_and(on_this_machine),
                driver,
                base_url,
                env: entry.map(|entry| entry.env.clone()).unwrap_or_default(),
                doc: entry.and_then(|entry| entry.doc.clone()),
                models: entry.map_or(0, |entry| entry.models.len()),
                supported,
            }
        })
        .collect()
}

/// 每一家的 key 的环境变量：`env` 只有一个名字的。一个名字几家用的，一家一条。照名字（不分大小写）、编号排。
pub fn key_vars(listed: &[Listed]) -> Vec<(String, &Listed)> {
    let mut vars: Vec<(String, &Listed)> = listed
        .iter()
        .filter_map(|entry| match entry.env.as_slice() {
            [only] => Some((only.clone(), entry)),
            _ => None,
        })
        .collect();
    vars.sort_by(|(_, a), (_, b)| by_name(a).cmp(&by_name(b)));
    vars
}

/// 找了哪些环境变量：名字，照字节排、去重。
pub fn looked_for(listed: &[Listed]) -> Vec<String> {
    let mut names: Vec<String> = key_vars(listed).into_iter().map(|(name, _)| name).collect();
    names.sort_unstable();
    names.dedup();
    names
}

/// 探本机的哪几家：能用、地址在本机的，照编号排。只探本机，不往外发（第七条第 2 条）。
pub fn local_services(listed: &[Listed]) -> Vec<&Listed> {
    listed
        .iter()
        .filter(|entry| entry.supported && entry.local)
        .collect()
}

/// `provider.catalog`：编号、名字里有 `query` 这一截的（不分大小写；不写、空的是全部），能用的在前，再照名字（不分大小写）、
/// 编号排，最多 `limit` 家。
pub fn search<'a>(listed: &'a [Listed], query: Option<&str>, limit: usize) -> Vec<&'a Listed> {
    let query = query.unwrap_or_default().to_lowercase();
    let mut found: Vec<&Listed> = listed
        .iter()
        .filter(|entry| {
            entry.id.to_lowercase().contains(&query) || entry.name.to_lowercase().contains(&query)
        })
        .collect();
    found.sort_by(|a, b| (!a.supported, by_name(a)).cmp(&(!b.supported, by_name(b))));
    found.truncate(limit);
    found
}

/// 照名字排的键：名字不分大小写，一样的照编号。
fn by_name(entry: &Listed) -> (String, &str) {
    (entry.name.to_lowercase(), entry.id.as_str())
}
