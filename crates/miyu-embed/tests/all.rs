//! `miyu-embed` 的测试（施工 R-5 上，`docs/blueprint/recall.md` 第四条）：分词照 Hugging Face 的结果逐个比，清单的读法，
//! 小程序的协议（手造的小模型，CI 里跑），真模型的向量（`#[ignore]`，照 `MIYU_EMBED_MODEL_DIR` 找文件，验收时跑）。

mod manifest;
mod protocol;
mod real;
mod support;
mod tokens;
