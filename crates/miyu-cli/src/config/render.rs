//! `miyu config` 印的几样（`config.md`「样子」）：值照 TOML 写，报错一行 `路径:行:列 级别：那一句`，`explain` 的第一行和
//! 每一层一行。只管写成什么样，不管往哪写。

use serde_json::Value;

use super::paths::Places;
use crate::language::Language;
use crate::shown::{Ink, Line};

/// `get` 印的：只问一个键的只印值（字不带引号）；别的一行一个 `键 = 值`，照键名排（`items` 本来就照键名排）。
pub(super) fn values(items: &Value, single: bool) -> String {
    let Some(items) = items.as_object() else {
        return String::new();
    };
    let mut text = String::new();
    for (key, item) in items {
        let value = &item["value"];
        let shown = match (single, value) {
            (true, Value::String(bare)) => bare.clone(),
            _ => toml(value),
        };
        match single {
            true => text.push_str(&shown),
            false => text.push_str(&format!("{key} = {shown}")),
        }
        text.push('\n');
    }
    text
}

/// 协议上的一个值写成 TOML：字写成双引号的字符串（照 TOML 转义），开关、数照原样，列表写成一行。
pub(super) fn toml(value: &Value) -> String {
    match value {
        Value::String(text) => miyu_config::Value::Text(text.clone().into()).toml(),
        Value::Array(items) => {
            let items: Vec<String> = items.iter().map(toml).collect();
            format!("[{}]", items.join(", "))
        }
        Value::Object(map) => {
            let pairs: Vec<String> = map
                .iter()
                .map(|(key, value)| format!("{key} = {}", toml(value)))
                .collect();
            format!("{{ {} }}", pairs.join(", "))
        }
        other => other.to_string(),
    }
}

/// 报错一行：`<文件>:<行>:<列> <级别>：<那一句>`，整份的问题没有行列。级别「错误」红、「警告」黄，别的原色。
pub(super) fn problem(file: &str, problem: &Value, language: Language) -> Line {
    let place = match (problem["line"].as_u64(), problem["column"].as_u64()) {
        (Some(line), Some(column)) => format!("{file}:{line}:{column} "),
        _ => format!("{file} "),
    };
    let level = problem["level"].as_str().unwrap_or("error");
    let ink = match level {
        "warning" => Ink::Yellow,
        _ => Ink::Red,
    };
    let mut line = Line::inked(Ink::Plain, place);
    line.push(ink, language.severity(level));
    line.push(
        Ink::Plain,
        format!(
            "{}{}",
            language.colon(),
            problem["message"].as_str().unwrap_or_default()
        ),
    );
    line
}

/// `explain` 印的：第一行名字、键、说明、什么时候生效；下面每一层一行，从上往下，几列照显示的宽度对齐。生效的那一行
/// 原色，别的灰；写了、不算的那一行末尾红字写原因。
pub(super) fn explain(
    key: &str,
    item: &Value,
    said: &Value,
    language: Language,
    places: &Places,
) -> Vec<Line> {
    let header = language.explain_header(
        said["name"].as_str().unwrap_or(key),
        key,
        said["description"].as_str().unwrap_or_default(),
        said["applies"].as_str().unwrap_or_default(),
    );
    let mut lines = vec![Line::inked(Ink::Plain, header)];
    let rows: Vec<Row> = item["layers"]
        .as_array()
        .map(|layers| {
            layers
                .iter()
                .map(|layer| Row::of(layer, language, places))
                .collect()
        })
        .unwrap_or_default();
    let widths = [
        rows.iter()
            .map(|row| columns(&row.value))
            .max()
            .unwrap_or(0),
        rows.iter()
            .map(|row| columns(&row.layer))
            .max()
            .unwrap_or(0),
        rows.iter()
            .map(|row| columns(&row.place))
            .max()
            .unwrap_or(0),
    ];
    for row in &rows {
        // 一行的几格：值、哪一层，有文件的再一格在哪；后面还有东西的格补齐到这一列最宽的，最后一格不补。
        let mut cells = vec![row.value.as_str(), row.layer.as_str()];
        if !row.place.is_empty() {
            cells.push(&row.place);
        }
        let last = cells.len() - 1;
        let text: Vec<String> = cells
            .iter()
            .enumerate()
            .map(|(at, cell)| match at == last && row.mark.is_none() {
                true => (*cell).to_string(),
                false => padded(cell, widths[at]),
            })
            .collect();
        let ink = match row.used {
            true => Ink::Plain,
            false => Ink::Gray,
        };
        let mut line = Line::inked(ink, format!("  {}", text.join("  ")));
        if let Some((mark_ink, mark)) = &row.mark {
            line.push(Ink::Plain, "  ".to_string());
            line.push(*mark_ink, mark.clone());
        }
        lines.push(line);
    }
    lines
}

/// `explain` 的一层。
struct Row {
    value: String,
    layer: String,
    place: String,
    used: bool,
    mark: Option<(Ink, String)>,
}

impl Row {
    fn of(layer: &Value, language: Language, places: &Places) -> Row {
        let origin = &layer["origin"];
        let name = origin["layer"].as_str().unwrap_or("default");
        let used = layer["used"].as_bool().unwrap_or(false);
        let env = name == "env";
        let layer_name = match (env, origin["name"].as_str()) {
            (true, Some(variable)) => format!("{} {variable}", language.layer_name(name)),
            _ => language.layer_name(name).to_string(),
        };
        let place = match (origin["file"].as_str(), origin["line"].as_u64()) {
            (Some(file), Some(line)) => format!("{}:{line}", places.shown(file)),
            _ => String::new(),
        };
        let mark = match (used, layer["problem"].as_str()) {
            (true, _) => Some((Ink::Plain, language.in_effect(env).to_string())),
            (false, Some(problem)) => Some((Ink::Red, language.not_counted(problem))),
            (false, None) => None,
        };
        Row {
            value: toml(&layer["value"]),
            layer: layer_name,
            place,
            used,
            mark,
        }
    }
}

/// 补空格补到 `width` 列宽。
fn padded(text: &str, width: usize) -> String {
    let pad = width.saturating_sub(columns(text));
    format!("{text}{}", " ".repeat(pad))
}

/// 在终端里占几列：中日韩的字、全角的标点占两列，别的一列（和帮助页的规矩一样）。
pub(super) fn columns(text: &str) -> usize {
    text.chars()
        .map(|c| match c {
            '\u{1100}'..='\u{115F}'
            | '\u{2E80}'..='\u{A4CF}'
            | '\u{AC00}'..='\u{D7A3}'
            | '\u{F900}'..='\u{FAFF}'
            | '\u{FE30}'..='\u{FE4F}'
            | '\u{FF00}'..='\u{FF60}'
            | '\u{FFE0}'..='\u{FFE6}' => 2,
            _ => 1,
        })
        .sum()
}
