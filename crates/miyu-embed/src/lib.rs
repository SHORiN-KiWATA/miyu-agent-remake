//! 本机 embedding 的小程序 `miyu-embed`（施工 R-5 上，`docs/blueprint/recall.md` 第四条）：照清单（[`manifest`]）读一份模型，
//! 照 WordPiece 切词（[`wordpiece`]），ONNX Runtime 算向量（[`model`]），标准输入一行一句、标准输出一行一个向量
//! （[`serve`]）。核心按需拉起它（R-5 中）；ONNX Runtime 静态链接在这里，主程序不带。

pub mod manifest;
pub mod model;
pub mod serve;
pub mod wordpiece;
