//! 头这边用的配置（蓝图 `tui.md`「界面语言」，核心 8-2、8-3、8-4）：读 `ui.language` 的最终值、写进个人设置、
//! 订阅配置流跟着变。

use std::io;

use serde_json::{Value, json};

use super::rpc::Rpc;

/// 界面语言的配置项。
const LANGUAGE: &str = "ui.language";

/// 连上以后：订阅配置流（别处改了推 `config.changed`），读一次界面语言。交回读的那条请求的编号。
pub(super) async fn follow(rpc: &mut Rpc) -> io::Result<String> {
    rpc.send("subscribe", json!({"stream": "config"})).await?;
    read_language(rpc).await
}

/// 读 `ui.language` 的最终值。交回请求编号。
pub(super) async fn read_language(rpc: &mut Rpc) -> io::Result<String> {
    rpc.send("config.get", json!({"keys": [LANGUAGE]})).await
}

/// `config.get` 的回应里界面语言的最终值；没有的是 `auto`。
pub(super) fn language(result: &Value) -> String {
    result["items"][LANGUAGE]["value"]
        .as_str()
        .unwrap_or("auto")
        .to_string()
}

/// 推来的 `config.changed` 动了界面语言。
pub(super) fn touches_language(params: &Value) -> bool {
    params["keys"].get(LANGUAGE).is_some()
}

/// 新会话默认用哪个（手动换的模型，`/model`）：写进个人设置的 `models.chat`。
pub(super) fn set_chat(reference: &str) -> Value {
    json!({"layer": "personal", "changes": [{"key": "models.chat", "value": reference}]})
}

/// 把一个模型的思考强度写进个人设置（`key` 照 `facts.effort.key` 抄）；`level` 是 `None` 的去掉这一项（回到供应商定）。
pub(super) fn set_effort(key: &str, level: Option<&str>) -> Value {
    let change = match level {
        Some(level) => json!({"key": key, "value": level}),
        None => json!({"key": key, "unset": true}),
    };
    json!({"layer": "personal", "changes": [change]})
}

/// 把界面语言写进个人设置（`code` 是 `auto` 或者语言代码）。
pub(super) fn set_language(code: &str) -> Value {
    json!({"layer": "personal", "changes": [{"key": LANGUAGE, "value": code}]})
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{language, set_language, touches_language};

    #[test]
    fn the_language_is_read_written_and_noticed_by_its_key() {
        let got = json!({"items":{"ui.language":{"origin":{"layer":"personal"},"value":"ja"}}});
        assert_eq!(language(&got), "ja");
        assert_eq!(language(&json!({"items":{}})), "auto", "没有的当自动");
        assert!(touches_language(
            &json!({"keys":{"ui.language":{"value":"en"}},"layer":"personal"})
        ));
        assert!(
            !touches_language(&json!({"keys":{},"layer":"personal"})),
            "只改了注释"
        );
        assert_eq!(
            set_language("auto"),
            json!({"layer":"personal","changes":[{"key":"ui.language","value":"auto"}]})
        );
    }
}
