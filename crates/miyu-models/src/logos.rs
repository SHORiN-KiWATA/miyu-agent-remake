//! 供应商的图标（施工 8-31，`docs/blueprint/models.md`「图标」）：哪一家从哪拉、拉到的收不收。纯逻辑：拉、存、交给头在
//! `miyu-core` 的 `models/logos.rs`、`miyu-endpoint`。
//!
//! - 来源照出厂的 `models/logos.json`：`colored` 里有的照 lobe-icons 的彩色版（照原样画），别的照 models.dev 的单色版（头照
//!   字色画）。两份都是 MIT，标志是各家的商标。
//! - models.dev 没有这一家的回一张默认图（200）：照一个不存在的编号拉一次记下，内容一样的当没有。
//! - 收的：UTF-8、不超过 [`LOGO_MAX`]、是一张 SVG（去掉前面的空白、XML 声明以后 `<svg` 开头，有 `</svg>`）。别的当没有。

use std::collections::BTreeMap;

use serde::Deserialize;

/// 一张图标最多多少字节：超了当没有（头照名字的第一个字画）。
pub const LOGO_MAX: usize = 32 * 1024;

/// 认默认图用的编号：models.dev 上没有这一家。
pub const PROBE_ID: &str = "miyu-no-such-provider";

/// 一家的图标。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Logo {
    /// SVG 的原文。
    pub svg: String,
    /// 单色的：头照字色画；彩色的照原样画。
    pub tint: bool,
}

/// 出厂的 `models/logos.json`。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogoTable {
    /// models.dev 的单色图，`{id}` 换成目录里的编号。
    pub models_dev: String,
    /// lobe-icons 钉住版本的地址，`{name}` 换成它的名字。
    pub lobe: String,
    /// 有彩色版的几家：目录里的编号 → lobe 的名字。
    pub colored: BTreeMap<String, String>,
}

impl LogoTable {
    /// 读 `models/logos.json`。
    ///
    /// # Errors
    ///
    /// 不是这个写法的，说哪里不对。
    pub fn parse(text: &str) -> Result<LogoTable, String> {
        serde_json::from_str(text).map_err(|error| format!("models/logos.json: {error}"))
    }

    /// 编号 `id` 那一家的图从哪拉、是不是单色的。
    pub fn source(&self, id: &str) -> (String, bool) {
        match self.colored.get(id) {
            Some(name) => (self.lobe.replace("{name}", name), false),
            None => (self.models_dev.replace("{id}", id), true),
        }
    }

    /// 认默认图要拉的那一张。
    pub fn probe(&self) -> String {
        self.models_dev.replace("{id}", PROBE_ID)
    }
}

/// 拉到的 `bytes` 收不收：收的交回 SVG 原文。`default` 是 models.dev 的默认图，内容一样的当没有。
pub fn accept(bytes: &[u8], default: Option<&[u8]>) -> Option<String> {
    if bytes.len() > LOGO_MAX || default.is_some_and(|default| default == bytes) {
        return None;
    }
    let text = std::str::from_utf8(bytes).ok()?;
    let mut head = text.trim_start();
    if head.starts_with("<?xml") {
        head = head[head.find("?>")? + 2..].trim_start();
    }
    (head.starts_with("<svg") && text.contains("</svg>")).then(|| text.to_string())
}

#[cfg(test)]
mod tests;
