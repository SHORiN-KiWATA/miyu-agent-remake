//! 读推送、读日志的几个小工具（施工 O-3 从 `mod.rs` 挪出来：那一份到了 500 行）。

use serde_json::{Value, json};

use super::Home;

/// 推送里的事件种类，照先后。
pub fn kinds(pushed: &[Value]) -> Vec<String> {
    pushed
        .iter()
        .filter(|push| push["method"] == json!("event"))
        .map(|push| {
            push["params"]["event"]["kind"]
                .as_str()
                .unwrap_or("?")
                .to_string()
        })
        .collect()
}

/// 推送里的事件，照先后（施工 3-8 六补）。
pub fn events(pushed: &[Value]) -> Vec<Value> {
    pushed
        .iter()
        .filter(|push| push["method"] == json!("event"))
        .map(|push| push["params"]["event"].clone())
        .collect()
}

/// 磁盘上会话 `session` 的日志，每条写成 JSON，照先后（施工 3-8 六补）：和推送里的比。
pub fn logged(home: &Home, session: &str) -> Vec<Value> {
    home.log(session)
        .iter()
        .map(|event| serde_json::from_str(&event.to_line()).expect("事件是 JSON"))
        .collect()
}
