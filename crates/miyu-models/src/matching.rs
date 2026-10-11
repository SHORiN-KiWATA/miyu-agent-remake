//! 四层对目录（`docs/blueprint/models.md`「怎么走」第二条第 4 到 7 条，「起草时定的」第 6 到 11 条，施工 8-7）：一家配好的
//! 供应商的一个模型，照先后对到目录里的一个条目，对上就停。
//!
//! 1. 手写指定：模型写了 `catalog = "cp/cm"` 的就是它；目录里没有的不往下猜，标出来（[`Found::Missing`]）。
//! 2. 供应商对上（[`recognize`]：手写的 `catalog`、编号、去掉分隔的编号、地址），在那一家里找一模一样的、规整以后一样的。
//! 3. 整个目录里名字一模一样的。
//! 4. 规整以后取最长的前缀（在 `-` 处断开）；只有一段、没有数字的通用名只在正好相等时算。
//!
//! 第 3、4 层几家同名的照第 6 条挑：先取第 2 层认出的那一家，再取原厂（[`Vendors`]），都没有的取编号照字节排第一的，价格
//! 不借。

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::catalog::{Catalog, Entry};

/// 规整一个模型名（第 5 条）：取最后一个 `/` 后面的；转成小写；空格、`_`、`.`、`-` 连成的一串换成一个 `-`；去掉头尾的
/// `-`。
pub fn normalize(name: &str) -> String {
    let last = name.rsplit('/').next().unwrap_or(name);
    let mut out = String::with_capacity(last.len());
    let mut gap = false;
    for c in last.chars() {
        if matches!(c, ' ' | '_' | '.' | '-') {
            gap = true;
            continue;
        }
        if gap && !out.is_empty() {
            out.push('-');
        }
        gap = false;
        out.extend(c.to_lowercase());
    }
    out
}

/// 认原厂的表（资源目录的 `models/vendors.toml`，核心读成 JSON 交进来）：家族的第一段 → 原厂在目录里的几个编号，照先后。
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct Vendors(BTreeMap<String, Vec<String>>);

impl Vendors {
    /// 读表。
    ///
    /// # Errors
    ///
    /// 不是「名字 → 编号的列表」：原话说是这张表。
    pub fn parse(json: &serde_json::Value) -> Result<Vendors, String> {
        Vendors::deserialize(json)
            .map_err(|error| format!("models/vendors.toml not readable: {error}"))
    }

    /// 这个条目的原厂：`family`（没有的用模型名）规整以后的第一段，去掉末尾的数字，查表。
    fn of(&self, family: Option<&str>, model: &str) -> &[String] {
        let normalized = normalize(family.unwrap_or(model));
        let first = normalized.split('-').next().unwrap_or_default();
        let stem = first.trim_end_matches(|c: char| c.is_ascii_digit());
        self.0.get(stem).map_or(&[], Vec::as_slice)
    }
}

/// 供应商是怎么认出来的（`model.list` 的 `catalog.how`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum How {
    /// 手写的 `catalog`。
    Config,
    /// 编号一样（不分大小写）。
    Id,
    /// 去掉分隔以后一样。
    SimilarId,
    /// 地址一样。
    Url,
}

impl How {
    /// 协议上的写法。
    pub fn as_str(self) -> &'static str {
        match self {
            How::Config => "config",
            How::Id => "id",
            How::SimilarId => "similar_id",
            How::Url => "url",
        }
    }
}

/// 一家配好的供应商在目录里是哪一家。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recognized {
    /// 目录里的编号。
    pub provider: String,
    /// 怎么认出来的。
    pub how: How,
}

/// 认一家配好的供应商（第 4 条第 2 层）：编号 `id`、地址 `base_url`、手写的 `catalog`。先对上的算：手写的（目录里没有的
/// 当认不出，不往下认）、编号一样、去掉 `-`、`_`、`.`、空格以后一样、地址一样（去掉末尾的 `/`、`/v1`，协议和主机名不分
/// 大小写）。几家都对上的取编号照字节排第一的。
pub fn recognize(
    catalog: &Catalog,
    id: &str,
    base_url: Option<&str>,
    written: Option<&str>,
) -> Option<Recognized> {
    let found = |how: How, test: &dyn Fn(&str, Option<&str>) -> bool| {
        catalog
            .providers()
            .find(|provider| test(&provider.id, provider.api.as_deref()))
            .map(|provider| Recognized {
                provider: provider.id.clone(),
                how,
            })
    };
    if let Some(written) = written {
        return catalog.provider(written).map(|_| Recognized {
            provider: written.to_string(),
            how: How::Config,
        });
    }
    let similar = squeezed(id);
    found(How::Id, &|other, _| other.eq_ignore_ascii_case(id))
        .or_else(|| found(How::SimilarId, &|other, _| squeezed(other) == similar))
        .or_else(|| {
            let address = address(base_url?);
            found(How::Url, &|_, api| {
                api.is_some_and(|api| self::address(api) == address)
            })
        })
}

/// 去掉 `-`、`_`、`.`、空格，转成小写。
fn squeezed(id: &str) -> String {
    id.chars()
        .filter(|c| !matches!(c, '-' | '_' | '.' | ' '))
        .flat_map(char::to_lowercase)
        .collect()
}

/// 地址比较的写法：去掉末尾的 `/`，再去掉末尾的 `/v1`，协议和主机名转成小写。
fn address(url: &str) -> String {
    let url = url.trim_end_matches('/');
    let url = url.strip_suffix("/v1").unwrap_or(url);
    let (scheme, rest) = url.split_once("://").unwrap_or(("", url));
    let (host, path) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
    format!(
        "{}://{}{path}",
        scheme.to_ascii_lowercase(),
        host.to_ascii_lowercase()
    )
}

/// 对上的条目。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Matched {
    /// 目录里的编号。
    pub provider: String,
    /// 目录里的模型名。
    pub model: String,
    /// 第几层对上的：1 到 4。
    pub layer: u8,
    /// 价格借不借：第 1、2 层借；第 3、4 层是认出的那一家、原厂的借，照字节序挑的不借。
    pub price: bool,
}

/// 对目录的结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Found {
    /// 对上了。
    Matched(Matched),
    /// 手写指定的条目目录里没有：不往下猜（「起草时定的」第 10 条）。
    Missing(String),
    /// 没对上：不借目录。
    Nothing,
}

/// 照四层对一个模型：`recognized` 是这家在目录里是哪一家（[`recognize`]），`model` 是模型名，`written` 是模型手写的
/// `catalog`。
pub fn find(
    catalog: &Catalog,
    vendors: &Vendors,
    recognized: Option<&str>,
    model: &str,
    written: Option<&str>,
) -> Found {
    if let Some(written) = written {
        return match written.split_once('/') {
            Some((provider, name)) if catalog.model(provider, name).is_some() => {
                Found::Matched(matched(provider, name, 1, true))
            }
            _ => Found::Missing(written.to_string()),
        };
    }
    let normalized = normalize(model);
    if let Some(entry) = recognized.and_then(|provider| catalog.provider(provider)) {
        let same = entry.models.contains_key(model).then_some(model);
        let similar = || {
            entry
                .models
                .keys()
                .find(|name| normalize(name) == normalized)
                .map(|name| &**name)
        };
        if let Some(name) = same.or_else(similar) {
            return Found::Matched(matched(&entry.id, name, 2, true));
        }
    }
    let named = catalog.named(model);
    if !named.is_empty() {
        return Found::Matched(pick(catalog, vendors, recognized, named, 3));
    }
    for prefix in prefixes(&normalized) {
        let generic = !prefix.contains('-') && !prefix.chars().any(|c| c.is_ascii_digit());
        if generic && prefix != normalized {
            continue;
        }
        let entries = catalog.normalized(prefix);
        if !entries.is_empty() {
            return Found::Matched(pick(catalog, vendors, recognized, entries, 4));
        }
    }
    Found::Nothing
}

/// 规整以后的名字本身，和它在每个 `-` 处断开的前缀，从长到短。
fn prefixes(normalized: &str) -> impl Iterator<Item = &str> {
    std::iter::once(normalized).chain(
        normalized
            .rmatch_indices('-')
            .map(move |(at, _)| &normalized[..at])
            .filter(|prefix| !prefix.is_empty()),
    )
}

/// 几家同名挑哪家（第 6 条）：认出的那一家；原厂（照条目照字节排的先后，取各自的原厂，第一个列了它的）；都没有的取第一个，
/// 价格不借。`entries` 照字节排、不是空的。
fn pick(
    catalog: &Catalog,
    vendors: &Vendors,
    recognized: Option<&str>,
    entries: &[Entry],
    layer: u8,
) -> Matched {
    if let Some(entry) = entries
        .iter()
        .find(|(provider, _)| Some(&**provider) == recognized)
    {
        return matched(&entry.0, &entry.1, layer, true);
    }
    let vendor = entries
        .iter()
        .flat_map(|(provider, model)| {
            let family = catalog
                .model(provider, model)
                .and_then(|entry| entry.family.as_deref());
            vendors.of(family, model)
        })
        .find_map(|vendor| entries.iter().find(|(provider, _)| **provider == **vendor));
    match vendor {
        Some((provider, model)) => matched(provider, model, layer, true),
        None => matched(&entries[0].0, &entries[0].1, layer, false),
    }
}

fn matched(provider: &str, model: &str, layer: u8, price: bool) -> Matched {
    Matched {
        provider: provider.to_string(),
        model: model.to_string(),
        layer,
        price,
    }
}

#[cfg(test)]
mod tests;
