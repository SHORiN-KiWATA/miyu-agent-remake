//! `human.get`（给人看的字，施工 W-1，`docs/blueprint/web-module.md`「二、给人看的字」）：工具的样子、说法的模板，
//! 核心经协议交出去，终端、网页都不用再自己去资源目录里读。
//!
//! 每次现读资源目录（`web-module.md`「起草时定的」第 32 条）：一个连接只要一次，读几份 JSON 很快；开发时改了资源，
//! 下一次调就是新的，不用重启核心。在阻塞线程里读。

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::{Map, Value, json};

use miyu_store::human::Human;

use crate::Core;
use crate::hello::Peer;
use crate::refusal::Refusal;

const TARGET: &str = "miyu::endpoint";

/// `human.get` 的参数。
#[derive(Debug, Deserialize)]
pub(crate) struct Params {
    /// 要哪种语言：2 到 8 个小写字母，例如 `zh`。不写照这个连接的语言。
    #[serde(default)]
    language: Option<String>,
}

/// `human.get`：给人看的字，工具的样子、说法的模板原文。回应里没有 `config` 那一格：配置的名字、说明在
/// `config.schema` 里（施工 8-2）。
pub(crate) async fn get(core: &Core, peer: Peer, params: Params) -> Result<Value, Refusal> {
    let language = params.language.unwrap_or_else(|| peer.language.to_string());
    if !is_language_code(&language) {
        return Err(Refusal::BAD_PARAMS);
    }
    let resources = core.resources.clone();
    let wanted = language.clone();
    let loaded = tokio::task::spawn_blocking(move || Human::load(&resources, &wanted)).await;
    let human = match loaded {
        Ok(Ok(human)) => human,
        Ok(Err(error)) => {
            tracing::warn!(target: TARGET, error = %error, "human not read");
            return Err(Refusal::INTERNAL);
        }
        Err(error) => {
            tracing::error!(target: TARGET, error = %error, "human panicked");
            return Err(Refusal::INTERNAL);
        }
    };
    let tools: Map<String, Value> = human
        .tools()
        .iter()
        .map(|(name, face)| (name.clone(), json!(face)))
        .collect();
    let said: BTreeMap<&str, &str> = human.said_entries().collect();
    Ok(json!({"language": language, "said": said, "tools": tools}))
}

/// `language` 合不合写法：2 到 8 个小写字母（`protocol.md`「`human.get`」）。
fn is_language_code(language: &str) -> bool {
    (2..=8).contains(&language.len()) && language.bytes().all(|b| b.is_ascii_lowercase())
}
