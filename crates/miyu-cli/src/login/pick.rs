//! `miyu login --list` 的表、交互着选的编号表、敲的字认成哪一个（`config.md`「样子」的 `miyu login`，施工 8-5）。几列照
//! 显示的宽度对齐。

use serde_json::Value;

use miyu_config::secret::valid_name;

use crate::config::columns;
use crate::language::Language;
use crate::shown::{Ink, Line};

/// 一个密钥的几样：名字、设没设、谁在用（连好的）。
fn fields(secret: &Value, language: Language) -> (String, bool, String) {
    let name = secret["name"].as_str().unwrap_or_default().to_string();
    let set = secret["set"] == Value::Bool(true);
    let used: Vec<&str> = secret["used_by"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    (name, set, language.users(&used))
}

/// 补空格补到 `width` 列宽。
fn padded(text: &str, width: usize) -> String {
    format!("{text}{}", " ".repeat(width.saturating_sub(columns(text))))
}

/// 几格字连成一行，列和列之间空两格，最后一列不补空格；整列都是空的（谁都没在用）不占位置。
pub(crate) fn row(cells: &[&str], widths: &[usize]) -> String {
    let mut parts: Vec<String> = cells
        .iter()
        .zip(widths)
        .filter(|(_, width)| **width > 0)
        .map(|(cell, width)| padded(cell, *width))
        .collect();
    if let Some(last) = parts.last_mut() {
        *last = last.trim_end().to_string();
    }
    parts.join("  ").trim_end().to_string()
}

/// 每一列最宽的。
pub(crate) fn widths(rows: &[Vec<String>]) -> Vec<usize> {
    let count = rows.iter().map(Vec::len).max().unwrap_or(0);
    (0..count)
        .map(|at| {
            rows.iter()
                .filter_map(|row| row.get(at))
                .map(|cell| columns(cell))
                .max()
                .unwrap_or(0)
        })
        .collect()
}

/// `--list`：一个一行，名字、已设置或未设置、谁在用；设了的在前（原色），没设的在后（灰字），各自照名字的先后。
pub(super) fn table(secrets: &[Value], language: Language) -> Vec<Line> {
    let mut listed: Vec<(String, bool, String)> = secrets
        .iter()
        .map(|secret| fields(secret, language))
        .collect();
    listed.sort_by_key(|(_, set, _)| !*set);
    let rows: Vec<Vec<String>> = listed
        .iter()
        .map(|(name, set, used)| {
            vec![
                name.clone(),
                language.key_state(*set).to_string(),
                used.clone(),
            ]
        })
        .collect();
    let widths = widths(&rows);
    listed
        .iter()
        .zip(&rows)
        .map(|((_, set, _), cells)| {
            let cells: Vec<&str> = cells.iter().map(String::as_str).collect();
            let ink = if *set { Ink::Plain } else { Ink::Gray };
            Line::inked(ink, row(&cells, &widths))
        })
        .collect()
}

/// 选的时候的编号表：`  编号  名字  谁在用  设没设`，照给的先后（核心照名字排好了）。
pub(super) fn numbered(secrets: &[Value], language: Language) -> Vec<Line> {
    let rows: Vec<Vec<String>> = secrets
        .iter()
        .enumerate()
        .map(|(at, secret)| {
            let (name, set, used) = fields(secret, language);
            vec![
                (at + 1).to_string(),
                name,
                used,
                language.key_state(set).to_string(),
            ]
        })
        .collect();
    let widths = widths(&rows);
    rows.iter()
        .map(|cells| {
            let cells: Vec<&str> = cells.iter().map(String::as_str).collect();
            Line::inked(Ink::Plain, format!("  {}", row(&cells, &widths)))
        })
        .collect()
}

/// 敲的字认成什么。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Chosen {
    /// 选了这一个：编号对上的，或者敲的合写法的名字。
    Name(String),
    /// 直接回车、读到头。
    Nothing,
    /// 既不是列出的编号，也不合名字的写法。
    Bad(String),
}

/// 敲的 `typed`（去掉了前后空白）认成哪一个：列出的编号，或者一个合写法的名字。
pub(super) fn chosen(typed: &str, secrets: &[Value]) -> Chosen {
    if typed.is_empty() {
        return Chosen::Nothing;
    }
    let numbered = typed
        .parse::<usize>()
        .ok()
        .and_then(|number| number.checked_sub(1))
        .and_then(|at| secrets.get(at))
        .and_then(|secret| secret["name"].as_str());
    match numbered {
        Some(name) => Chosen::Name(name.to_string()),
        None if valid_name(typed) => Chosen::Name(typed.to_string()),
        None => Chosen::Bad(typed.to_string()),
    }
}

#[cfg(test)]
mod tests;
