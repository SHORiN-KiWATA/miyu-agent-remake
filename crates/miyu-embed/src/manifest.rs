//! 读清单的文件（施工 R-5 上，`docs/blueprint/recall.md` 第四条第 2 款）。清单的类型 R-5 中挪进了 `miyu_recall::embedding`：
//! 核心也要读同一份，又不能依赖这个小程序（会把 ONNX Runtime 链进主程序）。这里只剩读文件那一步。

use std::path::Path;

pub use miyu_recall::embedding::{Manifest, ManifestError, ModelFile, Pooling, Role};

/// 读 `path` 那一份清单。
///
/// # Errors
///
/// 读不了（`cannot read <路径>: <原因>`），或者不合写法（[`Manifest::parse`]）。
pub fn read(path: &Path) -> Result<Manifest, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    Manifest::parse(&text).map_err(|error| error.to_string())
}
