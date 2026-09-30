//! 留痕（`docs/blueprint/config.md`「系统日志、账号日志」、「怎么走」第六条，施工 8-3）：改动落了盘，照事件的外壳追加
//! 一条。系统配置的进系统日志，个人设置的改动、项目配置的信任进账号日志。
//!
//! 先写配置，后写日志：配置文件是真相，日志是留痕。日志写不进去的记一条 `WARN journal not written`，配置照改、照回应。
//! `body` 的格照图纸的先后写，所以用带顺序的结构体写成 JSON，不经 `serde_json` 的 `Map`（它照字母排）。

use std::path::Path;

use serde::Serialize;

use miyu_kernel::id::{CommandId, EventKind};
use miyu_kernel::origin::By;
use miyu_kernel::raw::RawJson;
use miyu_kernel::time::Timestamp;
use miyu_store::journal::{self, Entry};

use super::TARGET;

/// `config.changed` 的 `body`：哪一层、哪个文件、怎么改的、这一层真变了的那几项。
#[derive(Debug, Serialize)]
pub(super) struct ConfigChanged<'a> {
    /// `system`、`personal`。
    pub(super) layer: &'a str,
    /// 给人看的写法，相对数据根。
    pub(super) file: &'a str,
    /// `set` 经 `config.set` 改几项，`edit` 整份换，`file` 手改被看到的（施工 8-4）。
    pub(super) via: &'a str,
    /// 变了的每一项，照键名排。
    pub(super) changes: Vec<KeyChange>,
}

/// 一项的改动：之前没写的不带 `old`，删掉的不带 `new`。
#[derive(Debug, Serialize)]
pub(super) struct KeyChange {
    /// 哪一项。
    pub(super) key: &'static str,
    /// 改之前这一层的值。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) old: Option<serde_json::Value>,
    /// 改之后的。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) new: Option<serde_json::Value>,
}

/// `trust.changed` 的 `body`：仓库在哪（`.miyu` 所在的那一层，写法同 `trust.toml`）、哪一份、信不信任。
#[derive(Debug, Serialize)]
pub(super) struct TrustChanged<'a> {
    /// 仓库在哪。
    pub(super) path: &'a str,
    /// 答的是哪一份。
    pub(super) version: &'a str,
    /// 信不信任。
    pub(super) trusted: bool,
    /// 手改 `trust.toml` 被看到的是 `file`（施工 8-4）；经 `config.trust` 记的不写。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) via: Option<&'a str>,
}

/// 往 `path`（给人看的写法是 `shown`）追加一条 `kind`，内容是 `body`：谁 `by`、哪个命令 `cause`（手改被看到的没有）、
/// 什么时候 `at`。写不进去的记一条 `WARN`，不往上报。
pub(super) fn record(
    path: &Path,
    shown: &str,
    at: miyu_kernel::time::Timestamp,
    by: By,
    cause: Option<&CommandId>,
    kind: &str,
    body: &impl Serialize,
) {
    if let Err(error) = append(path, at, by, cause, kind, body) {
        tracing::warn!(target: TARGET, file = %shown, error = %error, "journal not written");
    }
}

/// 写成一条、追加。
fn append(
    path: &Path,
    at: Timestamp,
    by: By,
    cause: Option<&CommandId>,
    kind: &str,
    body: &impl Serialize,
) -> Result<(), String> {
    let body = serde_json::to_string(body).map_err(|error| error.to_string())?;
    let entry = Entry {
        at,
        by,
        cause: cause.cloned(),
        kind: EventKind::parse(kind).map_err(|error| error.to_string())?,
        body: serde_json::from_str::<RawJson>(&body).map_err(|error| error.to_string())?,
    };
    journal::append(path, entry)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests;
