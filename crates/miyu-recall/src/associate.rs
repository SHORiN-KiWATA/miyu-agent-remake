//! 联想怎么挑（施工 R-8，`docs/blueprint/memory.md` 第四条）：人说的这一句照意思找到的记忆里，相似度过门槛的才带，宁可不带。
//! 门槛照模型：数据住在资源目录（`core/memory/recall.toml`），表里没有的模型不联想（没量过的不知道多像才算像）。
//!
//! 门槛是在测评集上量的（`docs/blueprint/memory/eval.md`）：bge-small-zh-v1.5 上平常的话最像的那一条相似度在 0.43 到 0.60
//! 之间，0.60 起平常的话一句都不误带。关键词那一路不参与：两字切分里「什么」「怎么」这类到处都有，加进来误带就上去了。

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;

/// 一轮最多带几条（`memory.md` 第四条第 4 款：每轮约 150 token）。
pub const TURN: usize = 3;

/// 最近一次压缩以后，联想累计最多带几条（第四条第 4 款：约 2000 token，一条约 50 token）。到了这一段不再带，压缩以后
/// 从零数。
pub const SEGMENT: usize = 40;

/// 照模型的门槛：模型照 `Vectors` 的写法（本机的 `local:<id>`，远程的 `<供应商>/<模型>`），相似度（点积，-1 到 1）不到
/// 门槛的不带。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Thresholds(BTreeMap<String, f32>);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Table {
    thresholds: BTreeMap<String, f64>,
}

impl Thresholds {
    /// 照 `recall.toml` 的原文读：一张 `[thresholds]`，模型 → 门槛。
    ///
    /// # Errors
    ///
    /// 不是合写法的 TOML、少了 `[thresholds]`、多写了别的表、门槛不是数或者不在 -1 到 1 之间：交回为什么（带上是哪一格）。
    pub fn parse(text: &str) -> Result<Thresholds, String> {
        let table: Table = toml_edit::de::from_str(text).map_err(|error| error.to_string())?;
        let mut read = BTreeMap::new();
        for (model, threshold) in table.thresholds {
            if !(-1.0..=1.0).contains(&threshold) {
                return Err(format!(
                    "thresholds.{model} = {threshold}：要在 -1 到 1 之间"
                ));
            }
            #[expect(
                clippy::cast_possible_truncation,
                reason = "门槛在 -1 到 1 之间，f32 放得下；相似度本来就是 f32"
            )]
            read.insert(model, threshold as f32);
        }
        Ok(Thresholds(read))
    }

    /// 模型 `model` 的门槛；表里没有的没有：不联想。
    #[must_use]
    pub fn of(&self, model: &str) -> Option<f32> {
        self.0.get(model).copied()
    }
}

/// 挑：`near` 是照这一句找到的（条目、相似度），最像的在前；交回要带的，照原来的先后。相似度不到 `threshold` 的不要，`seen`
/// 里的（这一段上下文里已经交过的）不要；最多 [`TURN`] 条，也不超过这一段剩下的名额（[`SEGMENT`] 减去已经带过的 `brought`）。
pub fn pick<Key: Ord + Clone>(
    near: &[(Key, f32)],
    threshold: f32,
    seen: &BTreeSet<Key>,
    brought: usize,
) -> Vec<Key> {
    let room = TURN.min(SEGMENT.saturating_sub(brought));
    near.iter()
        .filter(|(key, similar)| *similar >= threshold && !seen.contains(key))
        .map(|(key, _)| key.clone())
        .take(room)
        .collect()
}

#[cfg(test)]
mod tests;
