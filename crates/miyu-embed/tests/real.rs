//! 真模型（`recall.md` 第四条）：出厂的清单、Release 里的文件，向量和 Python 的 ONNX Runtime 算的（`fixtures/real.json`）
//! 余弦不低于 0.999。要先把 Release `models-bge-small-zh-v1.5` 的文件放进一个目录，`MIYU_EMBED_MODEL_DIR` 指到它：测试不
//! 联网，CI 里不跑，验收时跑（`cargo test -p miyu-embed -- --ignored`）。

use miyu_embed::manifest::Manifest;
use miyu_embed::model::Embedder;
use serde::Deserialize;

use crate::support::{cosine, read, shipped_manifest};

#[derive(Deserialize)]
struct Row {
    text: String,
    vector: Vec<f32>,
}

#[test]
#[ignore = "要真模型的文件：MIYU_EMBED_MODEL_DIR"]
fn the_real_model_agrees_with_the_reference() {
    let dir = std::env::var_os("MIYU_EMBED_MODEL_DIR").expect("设了 MIYU_EMBED_MODEL_DIR");
    let manifest = Manifest::read(&shipped_manifest()).expect("出厂的读得出");
    let mut embedder = Embedder::load(manifest, std::path::Path::new(&dir)).expect("载入得了");
    let rows: Vec<Row> = serde_json::from_str(&read("real.json")).expect("合写法");
    for row in rows {
        let got = embedder.embed(&row.text).expect("算得出");
        assert_eq!(got.len(), 512);
        let similar = cosine(&got, &row.vector);
        assert!(similar >= 0.999, "{}：余弦 {similar}", row.text);
    }
}
