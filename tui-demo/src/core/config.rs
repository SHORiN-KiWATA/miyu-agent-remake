//! 头这边用的配置（蓝图 `tui.md`「界面语言」，核心 8-2、8-3、8-4）：读 `ui.language`、`usage.currency`（金额哪种币排
//! 最前，核心 8-15）的最终值、写进个人设置、订阅配置流跟着变。

use std::io;

use serde_json::{Value, json};

use super::rpc::Rpc;

/// 界面语言的配置项。
const LANGUAGE: &str = "ui.language";
/// 金额哪种币排最前的配置项。
const CURRENCY: &str = "usage.currency";

/// 头要的几项配置的最终值。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadConfig {
    /// 界面语言：`auto` 或者语言代码。
    pub language: String,
    /// 金额排最前的币种，没有的是 `USD`。
    pub currency: String,
}

/// 连上以后：订阅配置流（别处改了推 `config.changed`），读一次界面语言。交回读的那条请求的编号。
pub(super) async fn follow(rpc: &mut Rpc) -> io::Result<String> {
    rpc.send("subscribe", json!({"stream": "config"})).await?;
    read(rpc).await
}

/// 读头要的几项的最终值。交回请求编号。
pub(super) async fn read(rpc: &mut Rpc) -> io::Result<String> {
    rpc.send("config.get", json!({"keys": [LANGUAGE, CURRENCY]}))
        .await
}

/// `config.get` 的回应里那几项；界面语言没有的是 `auto`，币种没有的是 `USD`。
pub(super) fn head(result: &Value) -> HeadConfig {
    let value = |key: &str, default: &str| {
        result["items"][key]["value"]
            .as_str()
            .unwrap_or(default)
            .to_string()
    };
    HeadConfig {
        language: value(LANGUAGE, "auto"),
        currency: value(CURRENCY, "USD"),
    }
}

/// 推来的 `config.changed` 动了头要的哪一项。
pub(super) fn touches(params: &Value) -> bool {
    [LANGUAGE, CURRENCY]
        .iter()
        .any(|key| params["keys"].get(key).is_some())
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

    use super::{head, set_language, touches};

    #[test]
    fn the_language_is_read_written_and_noticed_by_its_key() {
        let got = json!({"items":{"ui.language":{"origin":{"layer":"personal"},"value":"ja"}}});
        assert_eq!(head(&got).language, "ja");
        assert_eq!(head(&json!({"items":{}})).language, "auto", "没有的当自动");
        assert_eq!(head(&json!({"items":{}})).currency, "USD");
        let cny = json!({"items":{"usage.currency":{"value":"CNY"}}});
        assert_eq!(head(&cny).currency, "CNY");
        assert!(touches(
            &json!({"keys":{"ui.language":{"value":"en"}},"layer":"personal"})
        ));
        assert!(
            !touches(&json!({"keys":{},"layer":"personal"})),
            "只改了注释"
        );
        assert_eq!(
            set_language("auto"),
            json!({"layer":"personal","changes":[{"key":"ui.language","value":"auto"}]})
        );
    }
}
