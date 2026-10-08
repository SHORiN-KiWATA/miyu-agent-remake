//! 配置写读的五种拒绝（施工 8-2、8-3）：`refusal.rs` 到了 500 行，挪到这里。

use super::Refusal;

impl Refusal {
    /// 请求里写了清单里没有的配置项（施工 8-2，`config.schema`、`config.get`、`config.set`）：`data.problems` 里每个不认识的
    /// 一条。
    pub(crate) fn unknown_config_key(problems: Vec<serde_json::Value>) -> Refusal {
        Refusal::with(
            "unknown_config_key",
            "problems",
            serde_json::Value::Array(problems),
        )
    }

    /// `config.set` 的值不对、不能写在这一层，整份换的字里有错误（施工 8-3）：`data.problems` 里是每一处。
    pub(crate) fn config_invalid(problems: Vec<serde_json::Value>) -> Refusal {
        Refusal::with(
            "config_invalid",
            "problems",
            serde_json::Value::Array(problems),
        )
    }

    /// 文件现在读不进来，没法只改几项（施工 8-3）：`data.problems` 里是那几处。
    pub(crate) fn config_file_broken(problems: Vec<serde_json::Value>) -> Refusal {
        Refusal::with(
            "config_file_broken",
            "problems",
            serde_json::Value::Array(problems),
        )
    }

    /// `config.set` 的 `expect` 对不上（施工 8-3）：`data.current` 是这一层里这一项现在的样子，`{"value": …}` 或 `{}`。
    pub(crate) fn config_conflict_current(current: serde_json::Value) -> Refusal {
        Refusal::with("config_conflict", "current", current)
    }

    /// 版本对不上（施工 8-3）：整份换的、信任的那一份人看过以后又变了，写的那一瞬间有人手改了。`data.version` 是现在的
    /// 版本，文件没有的是 `null`。
    pub(crate) fn config_conflict_version(version: Option<String>) -> Refusal {
        Refusal::with("config_conflict", "version", serde_json::json!(version))
    }
}
