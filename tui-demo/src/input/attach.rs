//! 附件的规矩（蓝图 `tui.md`「输入框」第 12 条）：认哪几种、每一种认哪些扩展名、块上写什么。块怎么编号在
//! [`Editor::renumber`](super::Editor::renumber)。

use std::path::{Path, PathBuf};

/// 一种附件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachKind {
    /// `image`、`pdf`、`audio`、`video`。
    pub name: String,
    /// 认哪些扩展名：小写，不带点。
    pub extensions: Vec<String>,
    /// 块上写的：`{n}` 这一种的第几个。
    pub label: String,
}

/// 认得的几种附件。空的一种都不认（还没照配置设好时）。
#[derive(Debug, Clone, Default)]
pub struct AttachRule {
    /// 每一种，照登记的先后。
    pub kinds: Vec<AttachKind>,
}

impl AttachRule {
    /// 这个文件是哪一种：照扩展名认，不论大小写；认不出的是 `None`。
    pub fn kind_of(&self, path: &Path) -> Option<&str> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        self.kinds
            .iter()
            .find(|k| k.extensions.contains(&ext))
            .map(|k| k.name.as_str())
    }

    /// 这一种第 `n` 个的块上写的。没登记的种类写 `[种类 n]`。
    pub fn label(&self, kind: &str, n: usize) -> String {
        match self.kinds.iter().find(|k| k.name == kind) {
            Some(k) => k.label.replace("{n}", &n.to_string()),
            None => format!("[{kind} {n}]"),
        }
    }
}

/// 块里的附件：本机的哪个文件、是哪一种。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attachment {
    /// 本机的文件：发出去时交给核心读（`blob.put`）。
    pub file: PathBuf,
    /// 是哪一种（[`AttachKind::name`]）。
    pub kind: String,
}
