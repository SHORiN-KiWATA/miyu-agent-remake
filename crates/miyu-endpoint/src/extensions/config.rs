//! 包自己的配置（施工 9-4 下下，`docs/blueprint/extensions.md`「配置」）：核心拉起的扩展握手时交给它，之后变了推
//! `extension.config`。是 `config.md` 第九条「密钥不经协议交出去」的例外：只在核心亲手拉起、走标准输入输出的连接上，只给
//! 这个包自己的键（「包的编号加点」开头的），值不进运行日志、不进事件。

use std::collections::BTreeMap;

use serde_json::{Map, Value};

use crate::config::Config;

/// 包 `package` 自己的那份：键到值。值是最终值（没写的照默认值，系统配置、个人设置合出来的，不看项目配置），密钥引用解成
/// 真值的字；没设、没默认值、引用取不到的那一键不放。
pub(crate) fn own(config: &Config, package: &str) -> BTreeMap<String, Value> {
    let prefix = format!("{package}.");
    let resolved = config.resolved();
    config
        .items()
        .iter()
        .filter(|item| item.key.starts_with(&prefix) && !item.key.contains('<'))
        .filter_map(|item| {
            let (value, _) = resolved.get(item.key)?;
            let value = match value {
                miyu_config::Value::Secret(reference) => {
                    Value::String(config.secret(reference)?.expose().to_string())
                }
                other => other.json(),
            };
            Some((item.key.to_string(), value))
        })
        .collect()
}

/// 从 `before` 到 `after` 变了的键：新值，没了的是 `null`。没变的不放；都没变的是空的。
pub(crate) fn changes(
    before: &BTreeMap<String, Value>,
    after: &BTreeMap<String, Value>,
) -> Map<String, Value> {
    let mut changed = Map::new();
    for key in before.keys().chain(after.keys()) {
        let now = after.get(key);
        if now != before.get(key) && !changed.contains_key(key) {
            changed.insert(key.clone(), now.cloned().unwrap_or(Value::Null));
        }
    }
    changed
}

#[cfg(test)]
mod tests;
