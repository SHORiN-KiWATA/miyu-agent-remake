//! 页面文件（`web-module.md`「怎么走」第九条第 5、6 款）：`GET /` 给 `index.html`，别的照路径在页面目录里找。带 `..` 的、
//! 换成真实位置以后跑到页面目录外的、不是普通文件的，404。不要登录也给（「起草时定的」第 27 条）：里面没有秘密。

use std::path::{Path, PathBuf};

/// 请求的路径 `path`（不带 `?` 后面的）在页面目录 `pages` 里对应哪份文件。对不上的（带 `..`、跑出去、不是普通文件、
/// 读不了）是空的。
pub(crate) fn find(pages: &Path, path: &str) -> Option<PathBuf> {
    let path = decode(path)?;
    let relative = match path.trim_start_matches('/') {
        "" => "index.html",
        rest => rest,
    };
    let mut joined = pages.to_path_buf();
    for segment in relative.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." || segment.contains('\\') {
            return None;
        }
        joined.push(segment);
    }
    let real = std::fs::canonicalize(&joined).ok()?;
    let root = std::fs::canonicalize(pages).ok()?;
    (real.starts_with(&root) && real.is_file()).then_some(real)
}

/// 解开 `%xx`：解出来不是 UTF-8、有 NUL 的不要。
fn decode(path: &str) -> Option<String> {
    let bytes = path.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'%' {
            let hex = path.get(at + 1..at + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            at += 3;
        } else {
            out.push(bytes[at]);
            at += 1;
        }
    }
    let text = String::from_utf8(out).ok()?;
    (!text.contains('\0')).then_some(text)
}
