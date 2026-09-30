//! 附件和文件块的规矩（蓝图 `tui.md`「输入框」第 12 条）：认哪几种、每一种认哪些扩展名、块上写什么，文件块上的
//! 名字太长怎么截。附件的块怎么编号在 [`Editor::renumber`](super::Editor::renumber)。

use std::path::{Path, PathBuf};

use unicode_width::UnicodeWidthChar;

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

/// 认得的几种附件，和文件块怎么写。空的一种都不认（还没照配置设好时）。
#[derive(Debug, Clone, Default)]
pub struct AttachRule {
    /// 每一种，照登记的先后。
    pub kinds: Vec<AttachKind>,
    /// 文件块上写的：`{name}` 文件名。
    pub file: String,
    /// 目录块上写的：`{name}` 目录名。
    pub folder: String,
    /// 文件块上的名字最多几列，多的中间截掉。
    pub name_cols: usize,
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

    /// 文件块上写的：文件名（目录照目录的写法），太长的中间截掉（「输入框」第 12 条）。没有名字的（根目录）写整个路径。
    pub fn file_label(&self, path: &Path) -> String {
        let name = path.file_name().map_or_else(
            || path.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        let name = short_name(&name, self.name_cols);
        let template = if path.is_dir() {
            &self.folder
        } else {
            &self.file
        };
        template.replace("{name}", &name)
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

/// 名字超过 `max` 列的中间截掉写 `…`：结尾留扩展名再往前 3 列、至少留三分之一，开头拿剩下的；一个字放不下就不放，
/// 不超过 `max` 列（一个汉字两列）。
pub fn short_name(name: &str, max: usize) -> String {
    let width = |c: char| c.width().unwrap_or(0);
    if name.chars().map(width).sum::<usize>() <= max || max < 3 {
        return name.to_string();
    }
    let budget = max - 1;
    // 点在开头的（`.bashrc`）不算扩展名。
    let ext = name
        .rfind('.')
        .filter(|&at| at > 0)
        .map_or(0, |at| name[at..].chars().map(width).sum::<usize>());
    let tail_cols = (ext + 3).max(budget / 3).min(budget / 2 + budget % 2);
    let mut tail = Vec::new();
    let mut used = 0;
    for c in name.chars().rev() {
        if used + width(c) > tail_cols {
            break;
        }
        used += width(c);
        tail.push(c);
    }
    let head_cols = budget - used;
    let mut head = String::new();
    let mut head_used = 0;
    for c in name.chars() {
        if head_used + width(c) > head_cols {
            break;
        }
        head_used += width(c);
        head.push(c);
    }
    let tail: String = tail.into_iter().rev().collect();
    format!("{head}…{tail}")
}

#[cfg(test)]
mod tests {
    use unicode_width::UnicodeWidthStr;

    use super::short_name;

    #[test]
    fn long_names_lose_their_middle_and_keep_the_extension() {
        // 2026-09-30 项目主人：文件名长到在输入框里折行时，中间截掉、留扩展名，最多 24 列。
        let got = short_name("2026-09-30-项目周会纪要-终版-改3.docx", 24);
        assert_eq!(got, "2026-09-30-项目…改3.docx");
        assert_eq!(got.width(), 24);
        assert_eq!(short_name("notes.txt", 24), "notes.txt", "放得下的不动");
        let plain = short_name("abcdefghijklmnopqrstuvwxyz0123", 24);
        assert_eq!(
            plain, "abcdefghijklmnop…xyz0123",
            "没有扩展名的结尾留三分之一"
        );
        assert!(
            short_name("一二三四五六七八九十一二三四五六七八九十.md", 24).width() <= 24,
            "汉字放不下就不放"
        );
        let dot = short_name(".a-very-long-hidden-config-file-name", 24);
        assert!(
            dot.starts_with(".a-very") && dot.width() <= 24,
            "点在开头的不算扩展名：{dot}"
        );
    }
}
