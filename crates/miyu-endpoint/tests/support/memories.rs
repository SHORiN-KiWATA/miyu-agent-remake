//! 记忆的协议几个测试共用的（施工 R-3 补）：回应里的一条怎么读、连上一个核心。

use serde_json::Value;

use miyu_session::testkit::Script;

use std::sync::Arc;

use miyu_endpoint::Core;
use miyu_tool::Catalog;

use super::venues::configured_core;
use super::{Client, Home};

/// 一条记忆在回应里的样子：编号和正文。
pub fn texts(reply: &Value) -> Vec<(String, String)> {
    reply["result"]["memories"]
        .as_array()
        .unwrap_or_else(|| panic!("有 memories：{reply}"))
        .iter()
        .map(|memory| {
            (
                memory["id"].as_str().unwrap_or_default().to_string(),
                memory["text"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect()
}

pub fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(id, text)| (id.to_string(), text.to_string()))
        .collect()
}

/// 系统配置里默认人格是软件工程师的核心：施工 P-4 上起出厂不设默认人格，不带人格的会话、不写人格的 `memory.*` 记忆都不
/// 生效（17 L17）。
pub fn with_persona(home: &Home, script: &Script, tools: Catalog) -> Arc<Core> {
    home.write("system/config.toml", "[persona]\ndefault = \"engineer\"\n");
    configured_core(home, script, tools)
}

/// 连上 [`with_persona`] 的核心、握好手。
pub async fn connected(home: &Home, script: &Script) -> Client {
    let mut client = Client::connect(with_persona(home, script, Catalog::default()));
    client.hello().await;
    client
}
