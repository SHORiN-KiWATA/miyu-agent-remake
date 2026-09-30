//! 你说的话里的块（蓝图 `tui.md`「正文」第 2 条、「输入框」第 11、12 条）：粘贴块点开看全文；附件（图片、PDF、音频、
//! 视频）只写块上的字、不展开，点块用系统的程序打开那个文件，编号整个会话一直往下排。

use std::collections::HashMap;
use std::path::PathBuf;

use super::{Kind, Transcript};

/// 你说的话里的一块。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chip {
    /// 块上写的：`[已粘贴 14 行]`、`[图片 1]`。
    pub label: String,
    /// 原文：粘贴块点开换成它；附件的就是块上的字。
    pub full: String,
    /// 附件是哪一种（`image`、`pdf`、`audio`、`video`）；粘贴块是 `None`。
    pub kind: Option<String>,
    /// 附件是本机的哪个文件：点块用系统的程序打开它（「输入框」第 12 条）。粘贴块是 `None`。
    pub file: Option<PathBuf>,
}

impl Chip {
    /// 是附件：不展开，点块打开文件。
    pub fn attachment(&self) -> bool {
        self.kind.is_some()
    }
}

impl Transcript {
    /// 正文里你说的话还留着几个附件，每一种各数各的：撤销藏起来的不算，`/new` 清空了从头数（「输入框」第 12 条）。
    pub fn attachment_counts(&self) -> HashMap<String, usize> {
        let mut out = HashMap::new();
        let said = self
            .entries
            .iter()
            .filter(|e| e.kind == Kind::User && !e.hidden);
        for kind in said.flat_map(|e| &e.pasted).filter_map(|c| c.kind.as_ref()) {
            *out.entry(kind.clone()).or_default() += 1;
        }
        out
    }
}
