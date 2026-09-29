//! 撤销、恢复的回应里给人看的几样（`docs/blueprint/protocol/undo.md`「回应」）。这几样由核心算，头照着写。

use serde_json::Value;

/// 回应里给人看的几样。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    /// 撤了（恢复了）几轮。
    pub turns: u64,
    /// 第一轮里人说的那句话的第一行不空的；没有的是 `None`。
    pub said: Option<String>,
    /// 撤掉的几轮里真跑过几次执行命令的工具：命令改的撤不回。恢复时没有这一格。
    pub commands: u64,
    /// 改回了几个文件（`outcome` 是 `restored`）。
    pub restored: u64,
    /// 没改回的几个：之后被改过、没了、原处被占……（`outcome` 不是 `restored`）。
    pub untouched: u64,
}

impl Report {
    /// 从回应的 `result` 里读。读不懂的格当没有。
    pub fn read(result: &Value) -> Self {
        let files = result["files"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default();
        let restored = files.iter().filter(|f| f["outcome"] == "restored").count() as u64;
        Self {
            turns: result["turns"].as_u64().unwrap_or_default(),
            said: result["said"].as_str().map(str::to_string),
            commands: result["commands"].as_u64().unwrap_or_default(),
            restored,
            untouched: files.len() as u64 - restored,
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::Report;

    #[test]
    fn reads_the_sample_in_the_blueprint() {
        let result = json!({"commands": 2, "cwd": "/home/me/proj", "events": [14, 15],
            "files": [{"action": "write", "outcome": "restored", "path": "/a"},
                      {"action": "write", "outcome": "changed", "path": "/b", "diff": ["@@ -3 +3 @@"]}],
            "said": "把 README 改成中文", "turns": 1});
        assert_eq!(
            Report::read(&result),
            Report {
                turns: 1,
                said: Some("把 README 改成中文".into()),
                commands: 2,
                restored: 1,
                untouched: 1
            }
        );
    }
}
