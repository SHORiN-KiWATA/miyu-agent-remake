//! 去 QQ 取带的东西（施工 O-33，`onebot.md` 第一条「平台工具（二）」）：调哪个动作、回应里东西在哪。只拼参数、读回应，不下载
//! （下载在 `crate::media`）。照 NapCat 主线源码核过（「施工时定的」）：
//!
//! - `get_msg {message_id}`：编号数和字都收（`GetMsg.ts`），回应的 `message` 是段的数组，`group_id` 是群号（数或字）。
//! - `get_image {file}`、`get_file {file}`（`GetFile.ts`，`GetImage` 是同一个）：`file` 是段里的编号（图的 `file`、视频的 `file`、
//!   文件的 `file_id`），NapCat 先下到它那台机器上，回 `file`（它那台机器上的路径）、`url`（图、视频是下载地址，文件是本机路径）、
//!   `file_name`、`file_size`，开了 `enableLocalFile2Url` 的另有 `base64`。群文件也照 `get_file` 取（`get_group_file_url` 只回地址、
//!   要 packet 后端，不用）。

use std::path::PathBuf;

use serde_json::{Value, json};

use super::Fetch;

/// 东西在哪：照试的先后（2026-10-11 主会话定）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// 下载地址：只认 `http`、`https`。
    Url(String),
    /// 本机的路径：NapCat 和桥在同一台机器上才读得到（在 Docker 里的读不到）。
    Path(PathBuf),
    /// 内容的 base64（去掉了 `base64://`）。
    Base64(String),
}

/// 取平台编号是 `message_id` 的那一条消息的动作和参数：`get_msg {message_id}`，编号原样写成字。
pub fn get_msg(message_id: &str) -> (&'static str, Value) {
    ("get_msg", json!({"message_id": message_id}))
}

/// 取编号是 `id` 的那一样东西的动作和参数：图 `get_image {file}`，视频、文件 `get_file {file}`。
pub fn get_media(fetch: Fetch, id: &str) -> (&'static str, Value) {
    let action = match fetch {
        Fetch::Image => "get_image",
        Fetch::File => "get_file",
    };
    (action, json!({"file": id}))
}

/// `get_image`、`get_file` 回应的 `data` 里东西在哪，照先后：`url` 是 `http`、`https` 的地址；`file`、`url` 里的本机绝对路径
/// （`file://` 开头的去掉这一段），同一个只算一次；`base64`（`base64://` 开头的去掉这一段），空的不算。
pub fn sources(data: &Value) -> Vec<Source> {
    let mut found = Vec::new();
    let url = data["url"].as_str().map(str::trim).unwrap_or_default();
    let lower = url.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        found.push(Source::Url(url.to_string()));
    }
    for raw in [data["file"].as_str(), Some(url)].into_iter().flatten() {
        let raw = raw.trim();
        let path = PathBuf::from(raw.strip_prefix("file://").unwrap_or(raw));
        let source = Source::Path(path.clone());
        if path.is_absolute() && !found.contains(&source) {
            found.push(source);
        }
    }
    if let Some(encoded) = data["base64"].as_str().map(str::trim) {
        let encoded = encoded.strip_prefix("base64://").unwrap_or(encoded);
        if !encoded.is_empty() {
            found.push(Source::Base64(encoded.to_string()));
        }
    }
    found
}

/// 回应的 `data` 里的文件名（`file_name`），去掉首尾空白不空的才有。
pub fn file_name(data: &Value) -> Option<&str> {
    data["file_name"]
        .as_str()
        .map(str::trim)
        .filter(|name| !name.is_empty())
}

#[cfg(test)]
mod tests;
