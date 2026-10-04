//! `resources/web/web.json`（`web-module.md`「起草时定的」第 25 条）：网页软件的数放在它自己的资源里，不进核心的配置清单。

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use serde::Deserialize;

/// 文件在资源目录里的位置。
pub const FILE: &str = "web/web.json";

/// 读好的一份。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    /// 出厂的端口（`--port` 能换）。
    pub port: u16,
    /// 没有 WebSocket 连着、没有媒体在给，连续多少秒就退出。
    pub idle_seconds: u64,
    /// 页面的 `Content-Security-Policy`。
    pub csp: String,
    /// 扩展名（小写）→ 媒体类型。
    pub types: BTreeMap<String, String>,
}

impl Settings {
    /// 读资源目录 `resources` 里的 `web/web.json`。
    ///
    /// # Errors
    ///
    /// 读不了、不是这个形状：原话里说是哪个文件。
    pub fn load(resources: &Path) -> Result<Settings, String> {
        let path = resources.join(FILE);
        let text = std::fs::read_to_string(&path)
            .map_err(|error| format!("{} not readable: {error}", path.display()))?;
        serde_json::from_str(&text)
            .map_err(|error| format!("{} not readable: {error}", path.display()))
    }

    /// 空闲多久退出。
    pub fn idle(&self) -> Duration {
        Duration::from_secs(self.idle_seconds)
    }

    /// 路径 `path` 照扩展名的媒体类型；表里没有的是 `application/octet-stream`。
    pub fn type_of(&self, path: &Path) -> &str {
        path.extension()
            .and_then(|extension| extension.to_str())
            .and_then(|extension| self.types.get(&extension.to_ascii_lowercase()))
            .map_or("application/octet-stream", String::as_str)
    }
}
