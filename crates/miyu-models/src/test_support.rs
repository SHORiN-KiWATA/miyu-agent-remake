//! 几个模块的测试共用的：真目录裁出来的一份（`testdata/models-dev-trimmed.json`，施工 8-7 从 models.dev 的 `api.json`
//! 照「怎么走」第二条第 7 条那张表挑的几家，另加了一个窗口写成字的坏模型），认原厂的表（和资源目录的 `vendors.toml`
//! 一样）。

use std::collections::BTreeMap;

use miyu_config::merge::{Layers, Resolved, merge};
use miyu_config::parse::parse;
use miyu_config::{Item, Layer};
use serde_json::json;

use crate::Knowledge;
use crate::catalog::{Catalog, CatalogSource, Loaded};
use crate::matching::Vendors;
use crate::observed::{Learned, ProviderList};
use crate::profile::Profiles;
use crate::settings::{
    AuthCooldown, CatalogSettings, ModelSettings, PoolSettings, PriceSettings, ProviderSettings,
    RateLimitedCooldown, RetryableCooldown, UseSettings,
};

/// 裁出来的目录的原文。
pub(crate) const TRIMMED: &str = include_str!("../testdata/models-dev-trimmed.json");

/// 裁出来的目录。
pub(crate) fn trimmed() -> Catalog {
    Catalog::parse(TRIMMED).expect("读得进").catalog
}

/// 裁出来的目录，当作 2026-10-01 拉的快照。
pub(crate) fn loaded() -> Loaded {
    Loaded {
        catalog: trimmed(),
        source: CatalogSource::Snapshot,
        fetched: "2026-10-01T03:25:54.000Z".to_string(),
    }
}

/// 认原厂的表。
pub(crate) fn vendors() -> Vendors {
    Vendors::parse(&json!({
        "claude": ["anthropic"], "gpt": ["openai"], "o": ["openai"], "gemini": ["google"],
        "gemma": ["google"], "grok": ["xai"], "qwen": ["alibaba", "alibaba-cn"], "glm": ["zhipuai", "zai"],
        "kimi": ["moonshotai", "moonshotai-cn"], "deepseek": ["deepseek"], "mistral": ["mistral"],
        "minimax": ["minimax", "minimax-cn"], "mimo": ["xiaomi"]
    }))
    .expect("读得进")
}

/// 手里拿着的几份资料：查的时候借出 [`Knowledge`]。
pub(crate) struct Held {
    pub(crate) profiles: Profiles,
    pub(crate) vendors: Vendors,
    pub(crate) catalog: Option<Loaded>,
    pub(crate) learned: Learned,
    pub(crate) lists: BTreeMap<String, ProviderList>,
}

impl Held {
    /// 档案照 JSON 的 `profiles`，带上裁出来的目录（`catalog` 为真时），别的都是空的。
    pub(crate) fn new(profiles: serde_json::Value, catalog: bool) -> Held {
        Held {
            profiles: Profiles::parse(&profiles).expect("档案写法对"),
            vendors: vendors(),
            catalog: catalog.then(loaded),
            learned: Learned::default(),
            lists: BTreeMap::new(),
        }
    }

    /// 借出来。
    pub(crate) fn knowledge(&self) -> Knowledge<'_> {
        Knowledge {
            profiles: &self.profiles,
            vendors: &self.vendors,
            catalog: self.catalog.as_ref(),
            learned: &self.learned,
            lists: &self.lists,
        }
    }
}

/// 模型这一块的配置项。
pub(crate) fn items() -> Vec<Item> {
    [
        ProviderSettings::ITEMS,
        ModelSettings::ITEMS,
        PriceSettings::ITEMS,
        UseSettings::ITEMS,
        PoolSettings::ITEMS,
        CatalogSettings::ITEMS,
        RateLimitedCooldown::ITEMS,
        RetryableCooldown::ITEMS,
        AuthCooldown::ITEMS,
    ]
    .concat()
}

/// 当成系统配置读、合：写错的当场报出来。
pub(crate) fn resolved(source: &str) -> Resolved {
    let parsed = parse(&items(), Layer::System, source).expect("写法对");
    assert!(parsed.problems.is_empty(), "{:?}", parsed.problems);
    let layers = Layers {
        system: Some(&parsed),
        ..Layers::default()
    };
    merge(&items(), &layers, &|_| None)
}
