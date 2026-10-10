//! 页面文件（`web-ui.md`「怎么走」第一条第 5 款）：`GET /` 给 `index.html`，别的照路径在页面目录里找。带 `..` 的、换成真实
//! 位置以后跑到页面目录外的、不是普通文件的，404。不要登录也给（`web-module.md`「起草时定的」第 27 条）：里面没有秘密。
//! 类型照扩展名查各家给的表；响应头一律带 `nosniff`、`no-referrer`、`no-cache` 和各家给的内容安全策略。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use hyper::body::Bytes;
use hyper::header::{self, HeaderValue};
use hyper::{Method, Request, Response, StatusCode};

use crate::respond::{Body, empty, full, secure, value};

/// 给一个页面文件：页面目录 `pages`，内容安全策略 `csp`，扩展名（小写）到媒体类型的表 `types`。`GET`、`HEAD` 以外 405；
/// 找不到的、读不了的 404；`HEAD` 只给头。
pub async fn serve<B>(
    request: &Request<B>,
    pages: &Path,
    csp: &str,
    types: &BTreeMap<String, String>,
) -> Response<Body> {
    if request.method() != Method::GET && request.method() != Method::HEAD {
        return empty(StatusCode::METHOD_NOT_ALLOWED);
    }
    let Some(file) = find(pages, request.uri().path()) else {
        return empty(StatusCode::NOT_FOUND);
    };
    let Ok(bytes) = tokio::fs::read(&file).await else {
        return empty(StatusCode::NOT_FOUND);
    };
    let body = match request.method() == Method::HEAD {
        true => Bytes::new(),
        false => Bytes::from(bytes),
    };
    let mut response = Response::new(full(body));
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, value(type_of(types, &file)));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    headers.insert(header::CONTENT_SECURITY_POLICY, value(csp));
    secure(headers);
    response
}

/// 路径 `path` 照扩展名在表 `types` 里的媒体类型；表里没有的是 `application/octet-stream`。
pub fn type_of<'a>(types: &'a BTreeMap<String, String>, path: &Path) -> &'a str {
    path.extension()
        .and_then(|extension| extension.to_str())
        .and_then(|extension| types.get(&extension.to_ascii_lowercase()))
        .map_or("application/octet-stream", String::as_str)
}

/// 请求的路径 `path`（不带 `?` 后面的）在页面目录 `pages` 里对应哪份文件。对不上的（带 `..`、跑出去、不是普通文件、
/// 读不了）是空的。
pub fn find(pages: &Path, path: &str) -> Option<PathBuf> {
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

/// 解开 `%xx`：解出来不是 UTF-8、有 NUL 的不要（网页软件的软件后台页拆地址也用，施工 F-6 下）。
pub fn decode(path: &str) -> Option<String> {
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
