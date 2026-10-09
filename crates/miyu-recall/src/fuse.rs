//! 两路合并（施工 R-5 下，`docs/blueprint/recall.md` 第三条第 3 款）：关键词一路、向量一路各交一串名次，照名次合，不照分数
//! （bm25 和余弦的尺度对不上）：`分 = Σ 权重 / (60 + 名次 + 1)`，名次从 0 数。关键词为主、向量为辅（2026-09-29 项目主人定）：
//! 关键词一路权重 1，向量一路 0.5；只有向量一路找到的，相似度还要过 [`FLOOR`]。几个数是起点，测评集上再定。

use std::collections::BTreeMap;

/// RRF 的 `k`：名次靠后的分差变小（照通行的 60）。
const K: f32 = 60.0;

/// 关键词一路的权重。
const KEYWORD: f32 = 1.0;

/// 向量一路的权重：为辅。
const VECTOR: f32 = 0.5;

/// 只有向量一路找到的，相似度至少这么多。2026-10-09 在 bge-small-zh-v1.5 上量的起点：八句和记忆一个字都不重合的问题，对的
/// 那一条相似度最低 0.436、平均 0.564；不对的平均 0.297、九成在 0.368 以下、最高 0.456（施工单 R-5 下）。
pub const FLOOR: f32 = 0.40;

/// 合并：`keyword` 是关键词一路找到的，最相关的在前；`vector` 是向量一路的（条目、相似度），最像的在前。交回合并以后的条目，
/// 分高的在前；分一样的照关键词一路的先后、再照向量一路的。
pub fn fuse<Key: Ord + Clone>(keyword: &[Key], vector: &[(Key, f32)]) -> Vec<Key> {
    // 条目 → （分，第一次出现的先后）。
    let mut scored: BTreeMap<Key, (f32, usize)> = BTreeMap::new();
    let mut order = 0;
    for (rank, key) in keyword.iter().enumerate() {
        let entry = scored.entry(key.clone()).or_insert((0.0, order));
        entry.0 += share(KEYWORD, rank);
        order += 1;
    }
    for (rank, (key, similar)) in vector.iter().enumerate() {
        if *similar < FLOOR && !scored.contains_key(key) {
            continue;
        }
        let entry = scored.entry(key.clone()).or_insert((0.0, order));
        entry.0 += share(VECTOR, rank);
        order += 1;
    }
    let mut fused: Vec<(Key, (f32, usize))> = scored.into_iter().collect();
    fused.sort_by(|(_, (a, first_a)), (_, (b, first_b))| b.total_cmp(a).then(first_a.cmp(first_b)));
    fused.into_iter().map(|(key, _)| key).collect()
}

/// 名次 `rank`（从 0 数）在权重 `weight` 那一路分到的。
fn share(weight: f32, rank: usize) -> f32 {
    weight / (K + rank as f32 + 1.0)
}

#[cfg(test)]
mod tests;
