//! 几个测试共用的：数据在哪、读出来。

use std::path::PathBuf;

/// 测试数据的目录（`tests/fixtures/`）。
pub fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// 内置语义模型的包里的模型清单的原本（`package/embed/model.toml`，施工 R-5 三补）。
pub fn shipped_manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("package/embed/model.toml")
}

/// 读 `tests/fixtures/` 下的一份字。
pub fn read(name: &str) -> String {
    let path = fixtures().join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("读不了 {}：{error}", path.display()))
}

/// 两个向量的余弦。
pub fn cosine(a: &[f32], b: &[f32]) -> f64 {
    let dot: f64 = a
        .iter()
        .zip(b)
        .map(|(x, y)| f64::from(*x) * f64::from(*y))
        .sum();
    let norm = |v: &[f32]| v.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>().sqrt();
    dot / (norm(a) * norm(b))
}
