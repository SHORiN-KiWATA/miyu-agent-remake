//! `resources/software/mermaid/style.json`：字体、三种记号色、源码的上限、记几张（`web-module.md`「在哪」，
//! `mermaid.md`「怎么走」第 2 条，施工 W-4）。

use std::fmt;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::Marks;

/// 在资源目录里的位置。
fn file(resources: &Path) -> PathBuf {
    resources
        .join("software")
        .join("mermaid")
        .join("style.json")
}

/// `style.json` 的样子。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    fonts: Vec<String>,
    marks: MarksFile,
    max_source: usize,
    keep: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MarksFile {
    text: String,
    line: String,
    label: String,
}

/// 读好的一份。
#[derive(Debug, Clone)]
pub(crate) struct Style {
    /// 正文常用的几种字体，照先后找，最后一种是通用字体（例如 `sans-serif`）。
    pub(crate) fonts: Vec<String>,
    /// 字、线、连线标签垫底用的三种记号色：图里不会自己出现的颜色，头照它们换成自己的颜色。
    pub(crate) marks: Marks,
    /// 源码最长多少字节，去掉前后空白以后算。
    pub(crate) max_source: usize,
    /// 最多记几张画好的。
    pub(crate) keep: usize,
}

/// 这一份读不了，或者写法不对。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StyleError {
    file: PathBuf,
    why: String,
}

impl fmt::Display for StyleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.file.display(), self.why)
    }
}

impl std::error::Error for StyleError {}

/// 读 `resources` 下的 `software/mermaid/style.json`。
pub(crate) fn load(resources: &Path) -> Result<Style, StyleError> {
    let path = file(resources);
    let bad = |why: String| StyleError {
        file: path.clone(),
        why,
    };
    let text = std::fs::read_to_string(&path).map_err(|error| bad(error.to_string()))?;
    let parsed: File = serde_json::from_str(&text).map_err(|error| bad(error.to_string()))?;
    if parsed.fonts.is_empty() {
        return Err(bad("fonts 是空的".to_string()));
    }
    if parsed.max_source == 0 {
        return Err(bad("max_source 不能是 0".to_string()));
    }
    if parsed.keep == 0 {
        return Err(bad("keep 不能是 0".to_string()));
    }
    Ok(Style {
        fonts: parsed.fonts,
        marks: Marks {
            text: parsed.marks.text,
            line: parsed.marks.line,
            label: parsed.marks.label,
        },
        max_source: parsed.max_source,
        keep: parsed.keep,
    })
}

#[cfg(test)]
mod tests;
