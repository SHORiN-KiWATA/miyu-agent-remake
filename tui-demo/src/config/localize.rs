//! 写了两种语言的资源（蓝图 `tui.md`「界面语言」）：命令的说明、运行状态行的词在同一份 JSON 里写中英两种，读的时候
//! 挑界面语言那一种，读出来和只写一种的一样。

use serde::Deserialize;

use crate::language::Language;

/// 写了两种语言的（`{"zh": …, "en": …}` 这样只有语言做键的一格）挑 `language` 那一种，再照 `T` 读（命令的说明、
/// 运行状态行的词）。
pub(super) fn localized<T: for<'de> Deserialize<'de>>(
    name: &str,
    json: &str,
    language: Language,
) -> Result<T, String> {
    let mut value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("resources/{name} 读不懂：{e}"))?;
    pick(&mut value, language.code());
    serde_json::from_value(value).map_err(|e| format!("resources/{name} 读不懂：{e}"))
}

/// 一层层找只拿语言做键的对象，换成 `code` 那一种（没写的退回中文）。
fn pick(value: &mut serde_json::Value, code: &str) {
    match value {
        serde_json::Value::Object(map)
            if !map.is_empty() && map.keys().all(|k| k == "zh" || k == "en") =>
        {
            let chosen = map.get(code).or_else(|| map.get("zh")).cloned();
            if let Some(chosen) = chosen {
                *value = chosen;
            }
        }
        serde_json::Value::Object(map) => map.values_mut().for_each(|v| pick(v, code)),
        serde_json::Value::Array(list) => list.iter_mut().for_each(|v| pick(v, code)),
        _ => {}
    }
}
