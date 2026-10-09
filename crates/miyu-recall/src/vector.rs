//! 向量（施工 R-5 下，`docs/blueprint/recall.md` 第三条第 1、2 款）：检索库里存成 f32 的小端字节，长度是维数的 4 倍。存的
//! 都归一化过（`miyu-embed` 交回的就是），相似度就是点积。

/// 写成存进检索库的字节。
pub fn to_bytes(vector: &[f32]) -> Vec<u8> {
    vector
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

/// 从检索库的字节读回；长度不是 4 的倍数的读不出。
pub fn from_bytes(bytes: &[u8]) -> Option<Vec<f32>> {
    if !bytes.len().is_multiple_of(4) {
        return None;
    }
    Some(
        bytes
            .chunks_exact(4)
            .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect(),
    )
}

/// 两个归一化过的向量有多像：点积。维数对不上的（不是同一个模型的）算 0。
pub fn dot(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

#[cfg(test)]
mod tests;
