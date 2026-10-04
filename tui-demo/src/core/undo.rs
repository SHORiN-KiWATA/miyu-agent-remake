//! 撤销、恢复的回应里给人看的几样（`docs/blueprint/protocol/undo.md`「回应」）。这几样由核心算，头照着写。
//! 补发时没有回应，只有 `files.restored`：照它读出改回的每个文件，没有差异（蓝图 `tui.md`「正文」第 5 条）。

use serde_json::Value;

/// 改回的一个文件（回应的 `files`、`files.restored` 的 `files` 里的一项）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UndoFile {
    /// 真实的位置。
    pub path: String,
    /// 结局：`restored` 是改回了，别的都是没动（`changed`、`missing`、`occupied`、`gone`、`unsaved`、`unavailable`、
    /// `failed`）。
    pub outcome: String,
    /// 出错时系统的原话。
    pub error: Option<String>,
    /// 差异，几行统一格式的字（`@@ …`、`-…`、`+…`、` …`）；没有的是空的。
    pub diff: Vec<String>,
    /// 差异里没交出来的行数。
    pub more: u64,
    /// 加了几行、删了几行（核心 4-7 再补起给）；没给的照差异数。
    pub added: Option<u64>,
    /// 删了几行。
    pub removed: Option<u64>,
}

impl UndoFile {
    /// 从回应或者事件的一项里读；没有路径的是 `None`。
    pub fn read(item: &Value) -> Option<Self> {
        let lines = item["diff"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default();
        Some(Self {
            path: item["path"].as_str()?.to_string(),
            outcome: item["outcome"].as_str().unwrap_or_default().to_string(),
            error: item["error"].as_str().map(str::to_string),
            diff: lines
                .iter()
                .filter_map(|l| l.as_str().map(str::to_string))
                .collect(),
            more: item["more"].as_u64().unwrap_or_default(),
            added: item["added"].as_u64(),
            removed: item["removed"].as_u64(),
        })
    }

    /// 改回了。
    pub fn restored(&self) -> bool {
        self.outcome == "restored"
    }

    /// 读一串。
    pub fn read_all(list: &Value) -> Vec<Self> {
        list.as_array()
            .map(Vec::as_slice)
            .unwrap_or_default()
            .iter()
            .filter_map(Self::read)
            .collect()
    }
}

/// 回应里给人看的几样。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    /// 撤了（恢复了）几轮。
    pub turns: u64,
    /// 第一轮里人说的那句话的第一行不空的；没有的是 `None`。
    pub said: Option<String>,
    /// 工作目录：文件的路径照它写短。
    pub cwd: Option<String>,
    /// 改回的每个文件，照做的先后。
    pub files: Vec<UndoFile>,
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
        Self {
            turns: result["turns"].as_u64().unwrap_or_default(),
            said: result["said"].as_str().map(str::to_string),
            cwd: result["cwd"]
                .as_str()
                .filter(|c| !c.is_empty())
                .map(str::to_string),
            files: UndoFile::read_all(&result["files"]),
            compactions: result["compactions"].as_u64().unwrap_or_default(),
            clears: result["clears"].as_u64().unwrap_or_default(),
            jobs: result["jobs"].as_array().map_or(0, |j| j.len() as u64),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{Report, UndoFile};

    #[test]
    fn reads_the_sample_in_the_blueprint() {
        let result = json!({"commands": 2, "cwd": "/home/me/proj", "events": [14, 15],
            "files": [{"action": "write", "outcome": "restored", "path": "/a"},
                      {"action": "write", "outcome": "changed", "path": "/b", "diff": ["@@ -3 +3 @@"], "more": 4}],
            "said": "把 README 改成中文", "turns": 1, "compactions": 1, "clears": 1,
            "jobs": [{"job": "j1", "what": "command", "title": "跑测试"}]});
        let report = Report::read(&result);
        assert_eq!(report.turns, 1);
        assert_eq!(report.cwd.as_deref(), Some("/home/me/proj"));
        assert!(report.files[0].restored() && !report.files[1].restored());
        assert_eq!(
            report.files[1],
            UndoFile {
                path: "/b".into(),
                outcome: "changed".into(),
                diff: vec!["@@ -3 +3 @@".into()],
                more: 4,
                ..Default::default()
            }
        );
        assert_eq!((report.compactions, report.clears, report.jobs), (1, 1, 1));
    }
}
