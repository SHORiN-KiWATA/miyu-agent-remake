//! `miyu setup` 的编号表、敲的字认成什么（施工 8-11，`docs/blueprint/cli/setup.md`「怎么走」第 3、4、9 条、「样子」）。
//! 几列照显示的宽度对齐（和 `miyu login` 的表同一套，`login/pick.rs`）。

use crate::login::pick::{row, widths};
use crate::shown::{Ink, Line};

/// 表里的一行：几格字，选不选得了。选得了的从 1 编号；选不了的不编号、灰字。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Row {
    /// 几格字，不带编号。
    pub(super) cells: Vec<String>,
    /// 选得了。
    pub(super) usable: bool,
}

impl Row {
    /// 选得了的一行。
    pub(super) fn usable(cells: Vec<String>) -> Row {
        Row {
            cells,
            usable: true,
        }
    }
}

/// 编号表：每一行前面空两格；选得了的照先后从 1 编号，选不了的编号那一格空着、整行灰。`last` 是最后另加的一行（编号
/// 写死的，例如 `0  都不要`）：只和编号那一列对齐，字不占别的列的宽。
pub(super) fn numbered(rows: &[Row], last: Option<(&str, &str)>) -> Vec<Line> {
    let mut number = 0;
    let cells: Vec<Vec<String>> = rows
        .iter()
        .map(|entry| {
            let at = match entry.usable {
                true => {
                    number += 1;
                    number.to_string()
                }
                false => String::new(),
            };
            std::iter::once(at)
                .chain(entry.cells.iter().cloned())
                .collect()
        })
        .collect();
    let mut widths = widths(&cells);
    let mut lines: Vec<Line> = cells
        .iter()
        .zip(rows)
        .map(|(cells, entry)| {
            let cells: Vec<&str> = cells.iter().map(String::as_str).collect();
            let ink = if entry.usable { Ink::Plain } else { Ink::Gray };
            Line::inked(ink, format!("  {}", row(&cells, &widths)))
        })
        .collect();
    if let Some((at, text)) = last {
        // 最后一格不补空格，宽只要不是 0（整列都空的才不占位置）。
        let first = widths.first().copied().unwrap_or_default().max(at.len());
        widths = vec![first, 1];
        lines.push(Line::inked(
            Ink::Plain,
            format!("  {}", row(&[at, text], &widths)),
        ));
    }
    lines
}

/// 敲的一行认成什么。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Typed {
    /// 读到头了（管道关了、Ctrl+D）。
    End,
    /// 直接回车。
    Empty,
    /// `0`。
    Zero,
    /// 列出的第几个（从 0 数，只数选得了的）。
    Picked(usize),
    /// 别的字，去掉了前后空白。
    Other(String),
}

/// 敲的 `line`（读到头了的是空的）认成什么：列出的编号只到 `count`。
pub(super) fn typed(line: Option<String>, count: usize) -> Typed {
    let Some(line) = line else {
        return Typed::End;
    };
    let text = line.trim();
    match text.parse::<usize>() {
        _ if text.is_empty() => Typed::Empty,
        Ok(0) => Typed::Zero,
        Ok(number) if number <= count => Typed::Picked(number - 1),
        _ => Typed::Other(text.to_string()),
    }
}

/// 目录里的编号 `id` 在配置里写成什么（「施工时定的」）：合「路径里的名字」写法的（小写字母开头，只有小写字母、数字、`-`、
/// `_`）就是它；不合的（目录里的 `302ai`、`wafer.ai`）把别的字换成 `-`，不是字母开头的前面加 `p-`，写配置时另写
/// `catalog = "<id>"`。贴的 key 存成密钥也用这个名字。
pub(super) fn config_id(id: &str) -> String {
    let named: String = id
        .chars()
        .map(|c| match c {
            'a'..='z' | '0'..='9' | '-' | '_' => c,
            'A'..='Z' => c.to_ascii_lowercase(),
            _ => '-',
        })
        .collect();
    match named.starts_with(|c: char| c.is_ascii_lowercase()) {
        true => named,
        false => format!("p-{named}"),
    }
}

#[cfg(test)]
mod tests;
