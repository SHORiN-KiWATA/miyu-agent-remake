//! 执行命令、编辑那一块（施工 4-11，`docs/blueprint/cli/ask.md`「执行命令那一块」「编辑那一块」）：标题怎么写，
//! 标题下面印什么。照 opencode 的样子：命令写成 `$ 命令`，下面是她看到的输出；编辑下面是改掉的、改成的。

use serde_json::Value;

use super::{Drawn, Outcome};
use crate::language::Language;
use crate::shown::{Ink, Line, keep_tabs, strip_escapes};

/// 执行命令那一块：标题是 `<符号> <命令的第一行>`，不写显示名，不截；第二行起一行一行放在下面，前面写 `> `。
/// 结果是工具自己写的（`by_tool`），下面接着印结果里的字，标题不写结果那一句；不是的（被拒了、没跑就被打断了……），
/// 标题后面写结果那一句，下面不印。状态是 `error` 的，符号是红的。
pub(super) fn command(
    icon: &str,
    command: &str,
    body: &Value,
    by_tool: bool,
    outcome: &Outcome,
    language: &Language,
) -> Drawn {
    let ink = match outcome.status.as_str() {
        "error" => Ink::Red,
        _ => Ink::Plain,
    };
    let mut rows = command.lines();
    let mut title = Line::inked(ink, icon);
    if let Some(first) = rows.next() {
        title.push(Ink::Plain, format!(" {}", keep_tabs(first)));
    }
    let mut below: Vec<Line> = rows
        .map(|row| Line::inked(Ink::Plain, format!("> {}", keep_tabs(row))))
        .collect();
    match by_tool {
        true => below.extend(output(body)),
        false => outcome.tell(&mut title, language),
    }
    Drawn { title, below }
}

/// 结果里的字，她看到的原样：几块文字照先后接起来，一行一行印；终端的控制序列去掉；末尾的换行不多出一个空行。
/// 开头、末尾的空行不印，行首的缩进照留：一块前后的空行由印的这边管（施工 4-11 实测：`cargo test` 末尾自带一个
/// 空行，接上一块后面的空行，就空了两行）。
fn output(body: &Value) -> Vec<Line> {
    let text: String = body["blocks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|block| block["type"] == "text")
        .filter_map(|block| block["text"].as_str())
        .collect();
    let rows: Vec<String> = text.lines().map(strip_escapes).collect();
    let shown = |row: &&String| !row.trim().is_empty();
    let (Some(first), Some(last)) = (
        rows.iter().position(|row| shown(&row)),
        rows.iter().rposition(|row| shown(&row)),
    ) else {
        return Vec::new();
    };
    rows[first..=last]
        .iter()
        .map(|row| Line::inked(Ink::Plain, row.as_str()))
        .collect()
}

/// 编辑那一块：参数 `edits` 里的每一处，改掉的每一行前面写 `-`（红），改成的每一行前面写 `+`（绿），两处之间一行
/// `…`（灰）。某一处的 `old_string`、`new_string` 不是字符串的，那一处不印。
pub(super) fn edits(args: &Value) -> Vec<Line> {
    let mut below = Vec::new();
    for edit in args["edits"].as_array().into_iter().flatten() {
        let (Some(old), Some(new)) = (edit["old_string"].as_str(), edit["new_string"].as_str())
        else {
            continue;
        };
        if !below.is_empty() {
            below.push(Line::gray("…"));
        }
        below.extend(
            old.lines()
                .map(|row| Line::inked(Ink::Red, format!("-{}", keep_tabs(row)))),
        );
        below.extend(
            new.lines()
                .map(|row| Line::inked(Ink::Green, format!("+{}", keep_tabs(row)))),
        );
    }
    below
}

#[cfg(test)]
mod tests;
