//! `package.file`（施工 F-6 中，`package-pages.md`「`package.file`」）：读一个包的后台页目录里的一份文件，一次最多 512 KiB。
//! 媒体类型不给：网页软件照扩展名查它自己的那张表（同它给自己页面的那一套），核心不另放一份。

use std::path::{Path, PathBuf};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::Core;
use crate::refusal::Refusal;

/// `package.file` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FileParams {
    package: String,
    path: String,
    #[serde(default)]
    offset: u64,
}

/// 读一块：`{"data", "size", "eof"}`。
pub(crate) async fn read(core: &Core, params: FileParams) -> Result<Value, Refusal> {
    let segments = segments(&params.path).ok_or(Refusal::BAD_PARAMS)?;
    let dir = page_dir(core, &params.package)?;
    let offset = params.offset;
    tokio::task::spawn_blocking(move || {
        let real = beneath(&dir, &segments).ok_or(Refusal::FILE_NOT_FOUND)?;
        let segment = miyu_fs::read_range(&real, offset, miyu_fs::MAX_LENGTH)
            .map_err(|_| Refusal::FILE_NOT_FOUND)?;
        let eof = offset.saturating_add(segment.data.len() as u64) >= segment.size;
        Ok(json!({"data": STANDARD.encode(&segment.data), "size": segment.size, "eof": eof}))
    })
    .await
    .map_err(|_| Refusal::INTERNAL)?
}

/// 包 `id` 的后台页目录：没有这个包、没装的 `unknown_package`；没有后台页的 `no_page`。
fn page_dir(core: &Core, id: &str) -> Result<PathBuf, Refusal> {
    let packages = core.packages();
    let found = packages
        .iter()
        .find(|found| found.id == id)
        .ok_or(Refusal::UNKNOWN_PACKAGE)?;
    let manifest = found.read.as_ref().map_err(|_| Refusal::UNKNOWN_PACKAGE)?;
    match &manifest.page {
        Some(dir) if crate::packages::status::page(found, manifest) => {
            Ok(found.files_dir().join(dir))
        }
        _ => Err(Refusal::NO_PAGE),
    }
}

/// 相对路径拆成一段段：空的是 `index.html`；有空段、`.`、`..`、`\`、`:`、控制字符的不收。
pub(super) fn segments(path: &str) -> Option<Vec<String>> {
    if path.is_empty() {
        return Some(vec!["index.html".to_string()]);
    }
    path.split('/')
        .map(|segment| {
            let bad = segment.is_empty()
                || segment == "."
                || segment == ".."
                || segment.contains(['\\', ':'])
                || segment.chars().any(char::is_control);
            (!bad).then(|| segment.to_string())
        })
        .collect()
}

/// `dir` 下的这一份：换成真实的位置以后还在 `dir` 里、是普通文件的才算（链接指出去的不算）。
fn beneath(dir: &Path, segments: &[String]) -> Option<PathBuf> {
    let root = std::fs::canonicalize(dir).ok()?;
    let real = std::fs::canonicalize(
        segments
            .iter()
            .fold(dir.to_path_buf(), |path, segment| path.join(segment)),
    )
    .ok()?;
    (real.starts_with(&root) && real.is_file()).then_some(real)
}

#[cfg(test)]
mod tests;
