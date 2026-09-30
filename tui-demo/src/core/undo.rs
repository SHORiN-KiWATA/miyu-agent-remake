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
    /// 撤掉的几轮里有几次压缩（施工 6-9）：撤掉了压缩，上下文回到了压缩前。只有撤销有，是 0 的核心不写。
    pub compactions: u64,
    /// 撤掉的几轮里有几次清空（施工 6-8 补）：撤掉了清空，上下文回到了清空以前。是 0 的核心不写。
    pub clears: u64,
    /// 撤销时停掉了几个后台任务（回应的 `jobs`，施工 7-8）：撤掉的几轮派出去、那一刻还在跑的。
    pub jobs: u64,
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
            compactions: result["compactions"].as_u64().unwrap_or_default(),
            clears: result["clears"].as_u64().unwrap_or_default(),
            jobs: result["jobs"].as_array().map_or(0, |j| j.len() as u64),
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
            "said": "把 README 改成中文", "turns": 1, "compactions": 1, "clears": 1,
            "jobs": [{"job": "j1", "what": "command", "title": "跑测试"}]});
        assert_eq!(
            Report::read(&result),
            Report {
                turns: 1,
                said: Some("把 README 改成中文".into()),
                commands: 2,
                restored: 1,
                untouched: 1,
                compactions: 1,
                clears: 1,
                jobs: 1
            }
        );
    }
}
