//! 她上一轮的回答（`/copy`，蓝图「斜杠命令」`/copy`）。

use super::{Entry, Kind, Transcript};

impl Transcript {
    /// `/copy` 复制的：她上一轮的回答，Markdown 原文；一轮里被工具隔成几段的连起来、中间空一行，撤掉的不算
    /// （蓝图「斜杠命令」`/copy`）。还没有回答的是 `None`。
    pub fn last_reply(&self) -> Option<String> {
        let shown = |e: &&Entry| e.kind == Kind::Reply && !e.hidden && !e.text.trim().is_empty();
        let last = self.entries.iter().rev().find(shown)?;
        let parts: Vec<&str> = match last.turn {
            Some(turn) => self
                .entries
                .iter()
                .filter(shown)
                .filter(|e| e.turn == Some(turn))
                .map(|e| e.text.trim())
                .collect(),
            None => vec![last.text.trim()],
        };
        Some(parts.join("\n\n"))
    }
}
