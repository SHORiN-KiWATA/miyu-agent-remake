//! 读目录（`docs/blueprint/models.md`「怎么走」第二条第 1、2 条，施工 8-7）：安装包带的快照、缓存目录里后台拉的，两份比
//! `meta` 的 `fetched`，用新的；新的读不了，用另一份；都读不了，目录是空的，照样起来。
//!
//! 两份都是原样的 `api.json`，旁边一份 `<名字>.meta.json`：`{"source":<网址>,"fetched":<时刻>}`，缓存的另带 `etag`。
//! `meta` 没有、读不了的当作最旧。

use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::{Deserialize, Serialize};

use miyu_models::catalog::{Catalog, CatalogSource, Loaded};

use crate::TARGET;

/// 目录的文件名。
pub const FILE: &str = "models-dev.json";

/// 旁边那一份的文件名。
pub const META: &str = "models-dev.meta.json";

/// 一份目录旁边的 `meta`。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Meta {
    /// 从哪拉的。
    pub source: String,
    /// 什么时候拉的：RFC 3339 的 UTC 时刻，照字的先后就是时间的先后。
    pub fetched: String,
    /// 上次回的 `ETag`：缓存的才有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub etag: Option<String>,
}

impl Meta {
    /// 读 `dir` 里的 `meta`；没有的、坏的是空的。
    pub fn read(dir: &Path) -> Option<Meta> {
        let text = std::fs::read_to_string(dir.join(META)).ok()?;
        serde_json::from_str(&text).ok()
    }

    /// 写成 JSON。
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

/// 两份目录在哪：快照的目录（资源目录的 `models/`），缓存的目录（`<缓存目录>/models`，算不出来的没有）。
#[derive(Debug, Clone)]
pub struct Places {
    /// 快照所在的目录。
    pub snapshot: PathBuf,
    /// 缓存所在的目录。
    pub cache: Option<PathBuf>,
}

/// 读目录：挑新的、坏的退回另一份，读完记 `INFO catalog loaded`，坏了的记 `WARN catalog unreadable`，都读不了再记
/// `WARN catalog empty`。一个模型坏了的记一行 `DEBUG`。在阻塞线程里调。
pub fn load(places: &Places) -> Option<Loaded> {
    let mut candidates = vec![(CatalogSource::Snapshot, places.snapshot.clone())];
    if let Some(cache) = &places.cache {
        candidates.push((CatalogSource::Cache, cache.clone()));
    }
    let mut candidates: Vec<(CatalogSource, PathBuf, Option<Meta>)> = candidates
        .into_iter()
        .map(|(source, dir)| {
            let meta = Meta::read(&dir);
            (source, dir, meta)
        })
        .collect();
    // 新的在前：`fetched` 大的；一样新的快照在前（排序是稳定的）。
    candidates.sort_by(|a, b| fetched(&b.2).cmp(fetched(&a.2)));
    for (source, dir, meta) in candidates {
        let started = Instant::now();
        let read = std::fs::read_to_string(dir.join(FILE))
            .map_err(|error| error.to_string())
            .and_then(|text| Catalog::parse(&text));
        match read {
            Ok(read) => {
                for skipped in &read.skipped {
                    tracing::debug!(target: TARGET, entry = %skipped, "catalog entry skipped");
                }
                let fetched = meta.map(|meta| meta.fetched).unwrap_or_default();
                let catalog = read.catalog;
                tracing::info!(
                    target: TARGET,
                    source = source.as_str(),
                    fetched = %fetched,
                    providers = catalog.providers().count(),
                    models = catalog.model_count(),
                    ms = started.elapsed().as_millis(),
                    "catalog loaded"
                );
                return Some(Loaded {
                    catalog,
                    source,
                    fetched,
                });
            }
            Err(error) => {
                tracing::warn!(target: TARGET, source = source.as_str(), error = %error, "catalog unreadable");
            }
        }
    }
    tracing::warn!(target: TARGET, "catalog empty");
    None
}

/// 比新旧用的：没有 `meta` 的是空字，最旧。
fn fetched(meta: &Option<Meta>) -> &str {
    meta.as_ref().map_or("", |meta| meta.fetched.as_str())
}
