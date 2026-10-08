//! 自定义的一家（施工 8-11 再补，2026-10-08 项目主人定，`docs/blueprint/cli/setup.md`「怎么走」第 4 条）：依次问 base URL、
//! 接口协议（核心会说的三种），key 在试的时候问（可以空：本机的服务、不要 key 的中转）。配置里的编号照地址的主机名起
//! （[`host_id`]），撞了往后加 `-2`、`-3`（[`Flow::fresh_id`]）：能定的不让人填。

use serde_json::json;

use super::flow::{Chosen, Custom, Flow, Key};
use super::pick::{Row, Typed, config_id, numbered, typed};
use crate::shown::{say, write};

/// 接口协议：驱动的写法，照这个先后列。
const DRIVERS: [&str; 3] = ["openai-chat", "anthropic", "openai-responses"];

/// 主机名里这几段是通用的二级域名：编号往前取一段（`api.example.co.uk` 是 `example`）。
const SECOND_LEVEL: [&str; 7] = ["co", "com", "net", "org", "edu", "gov", "ac"];

impl Flow<'_> {
    /// 问 base URL、接口协议。读到头、直接回车的是「没选」。
    pub(super) fn custom(&mut self) -> Result<Chosen, u8> {
        let language = self.plan.language;
        let base_url = loop {
            let Some(line) = self.ask(language.base_url()) else {
                return self.not_picked();
            };
            let url = line.trim().trim_end_matches('/').to_string();
            if url.is_empty() {
                return self.not_picked();
            }
            if url.starts_with("http://") || url.starts_with("https://") {
                break url;
            }
            say(self.err, language.not_a_url());
        };
        say(self.err, language.protocol_heading());
        let rows: Vec<Row> = DRIVERS
            .iter()
            .map(|driver| Row::usable(vec![language.protocol(driver).to_string()]))
            .collect();
        for line in numbered(&rows, None) {
            write(self.err, &line.paint(self.plan.gray));
        }
        let driver = loop {
            match typed(self.ask(language.pick_number()), DRIVERS.len()) {
                Typed::Picked(at) => break DRIVERS[at],
                Typed::Empty | Typed::End => return self.not_picked(),
                Typed::Zero => say(self.err, &language.not_a_number("0")),
                Typed::Other(text) => say(self.err, &language.not_a_number(&text)),
            }
        };
        Ok(Chosen {
            id: host_id(&base_url),
            name: host(&base_url).to_string(),
            key: Key::Optional,
            custom: Some(Custom {
                base_url,
                driver: driver.to_string(),
            }),
        })
    }

    /// 配置里还没用过的编号：`id` 没用过的就是它，用过的往后加 `-2`、`-3`。照 `config.get` 的最终值。
    pub(super) async fn fresh_id(&mut self, id: &str) -> Result<String, u8> {
        let got = self.request("config.get", json!({})).await?;
        let taken = |candidate: &str| {
            let prefix = format!("providers.{candidate}.");
            got["items"]
                .as_object()
                .is_some_and(|items| items.keys().any(|key| key.starts_with(&prefix)))
        };
        Ok(std::iter::once(id.to_string())
            .chain((2..).map(|n| format!("{id}-{n}")))
            .find(|candidate| !taken(candidate))
            .unwrap_or_else(|| id.to_string()))
    }
}

/// 地址里的主机名：去掉协议、用户名、端口、路径。
pub(super) fn host(base_url: &str) -> &str {
    let rest = base_url
        .split_once("://")
        .map_or(base_url, |(_, rest)| rest);
    let authority = rest.split('/').next().unwrap_or_default();
    let authority = authority.rsplit('@').next().unwrap_or_default();
    match authority.strip_prefix('[') {
        Some(v6) => v6.split(']').next().unwrap_or_default(),
        None => authority.split(':').next().unwrap_or_default(),
    }
}

/// 照主机名起的编号：本机的、IP 的是 `local`；别的取倒数第二段（`api.example.com` 是 `example`），通用的二级域名再往前
/// 一段；写成配置里的名字（[`config_id`]）。
pub(super) fn host_id(base_url: &str) -> String {
    let host = host(base_url).to_ascii_lowercase();
    let numeric = host.chars().all(|c| c.is_ascii_digit() || c == '.');
    if host.is_empty() || host == "localhost" || host.contains(':') || numeric {
        return "local".to_string();
    }
    let labels: Vec<&str> = host.split('.').filter(|label| !label.is_empty()).collect();
    let picked = match labels.len() {
        0 => "custom",
        1 => labels[0],
        n if n >= 3 && SECOND_LEVEL.contains(&labels[n - 2]) => labels[n - 3],
        n => labels[n - 2],
    };
    config_id(picked)
}

#[cfg(test)]
mod tests;
