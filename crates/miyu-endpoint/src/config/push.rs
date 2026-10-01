//! 推送 `config.changed`（`docs/blueprint/config.md`「协议」「推送 `config.changed`」，施工 8-4）：系统配置、个人设置每变
//! 一次，推给订阅着配置的连接。
//!
//! 配置服务换上新的一份时造一个 [`Changed`]，带着那一刻的整份配置；每个连接的转发任务照自己的语言写成一行（问题的话、
//! 名字照连接的语言），所以推的是结构、不是写好的字。`keys` 的样子和 `config.set` 的回应一样（[`keys`]）。

use std::sync::Arc;

use serde::Serialize;
use serde_json::{Map, Value, json};

use miyu_config::Layer;
use miyu_kernel::origin::By;
use miyu_store::human::Human;

use super::methods::said;
use super::{Config, wire};
use crate::refusal::Refusal;

/// 怎么改的。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Via {
    /// 经 `config.set` 改几项。
    Set,
    /// 经 `config.set` 整份换。
    Edit,
    /// 手改，核心看到文件变了。
    File,
}

impl Via {
    /// 协议、日志上的写法。
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Via::Set => "set",
            Via::Edit => "edit",
            Via::File => "file",
        }
    }
}

/// 一次改动：推给订阅着配置的头。
#[derive(Debug)]
pub(crate) struct Changed {
    /// 改完以后的整份配置：最终值、来源、问题照它。
    pub(crate) config: Arc<Config>,
    /// 哪一层：系统配置、个人设置。
    pub(crate) layer: Layer,
    /// 怎么改的。
    pub(crate) via: Via,
    /// 谁改的：`via` 是 `set`、`edit` 才有。
    pub(crate) by: Option<By>,
    /// 这一层变了的项，照键名排。
    pub(crate) keys: Vec<&'static str>,
}

/// `config.changed` 这一行的样子：格照字母先后，`by` 照内核的写法（`kind` 在最前），所以用带顺序的结构体。
#[derive(Serialize)]
struct Line<'a> {
    jsonrpc: &'static str,
    method: &'static str,
    params: Params<'a>,
}

#[derive(Serialize)]
struct Params<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    by: Option<&'a By>,
    keys: Map<String, Value>,
    layer: &'static str,
    problems: Vec<Value>,
    version: Option<&'a str>,
    via: &'static str,
}

impl Changed {
    /// 写成一行通知，给人看的话照 `words`。
    ///
    /// # Errors
    ///
    /// 要用的字缺了：内部出错（已经记了 `WARN`）。
    pub(crate) fn line(&self, words: &Human) -> Result<String, Refusal> {
        let config = &*self.config;
        let file = config.file(self.layer);
        let layers = config.layers(None);
        let mut problems = Vec::new();
        let missing = config.missing(&file.parsed, self.layer);
        for problem in file.problems().chain(&missing) {
            problems.push(said(config, &layers, problem, Some(file), words)?);
        }
        let line = Line {
            jsonrpc: "2.0",
            method: "config.changed",
            params: Params {
                by: self.by.as_ref(),
                keys: keys(config, self.layer, &self.keys),
                layer: self.layer.as_str(),
                problems,
                version: file.version.as_deref(),
                via: self.via.as_str(),
            },
        };
        serde_json::to_string(&line).map_err(|_| Refusal::INTERNAL)
    }
}

/// 这一层变了的几项写成协议上的样子（`config.set` 的回应、推送共用）：什么时候生效、最终值和来源（不算项目配置）、这一层
/// 现在写的值（删掉的不写）。
pub(super) fn keys(config: &Config, layer: Layer, changed: &[&str]) -> Map<String, Value> {
    let shown = |at: Layer| Some(config.file(at).shown.clone());
    let file = config.file(layer);
    let mut listed = Map::new();
    for key in changed {
        let Some(item) = config.items().iter().find(|item| item.key == *key) else {
            continue;
        };
        let mut entry = Map::new();
        entry.insert("applies".to_string(), json!(item.applies.as_str()));
        if let Some((value, origin)) = config.resolved().get(key) {
            entry.insert("effective".to_string(), value.json());
            entry.insert("origin".to_string(), wire::origin(origin, &shown));
        }
        if let Some(written) = file.parsed.entries.get(key).filter(|entry| entry.counts) {
            entry.insert("value".to_string(), written.value.json());
        }
        listed.insert((*key).to_string(), Value::Object(entry));
    }
    listed
}
