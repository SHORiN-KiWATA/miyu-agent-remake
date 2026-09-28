//! 照核心交回的几样写成一行行给人看的（样子是项目主人 2026-09-28 定的，`docs/construction/4-7-miyu undo、miyu
//! redo（下）.md`）：第一行说撤的是哪一轮，每个文件一行，没动的同一行写原因，之后又被改过的下面印差异，执行过命令的
//! 说一句撤不回，撤销的最后说怎么恢复。只管写成什么样，不管往哪写。

use serde_json::Value;

use miyu_store::human::clean;

use super::{Direction, UndoPlan};
use crate::shown::{Ink, Line, cut, cut_front, keep_tabs, shown};

/// 那一轮人说的话最多印几个字。
const SAID_CHARS: usize = 40;
/// 路径最多印几个字。
const PATH_CHARS: usize = 80;
/// 出错时系统的原话最多印几个字。
const ERROR_CHARS: usize = 120;
/// 差异那几行缩进几格。
const INDENT: &str = "    ";

/// 核心交回的 `result` 写成的几行。
pub(super) fn lines(result: &Value, plan: &UndoPlan) -> Vec<Line> {
    let language = &plan.language;
    let said = result["said"]
        .as_str()
        .map(|said| cut(&clean(said), SAID_CHARS));
    let turns = result["turns"]
        .as_u64()
        .and_then(|turns| usize::try_from(turns).ok())
        .unwrap_or(1);
    let mut lines = vec![Line::gray(language.undo_header(
        plan.direction,
        said.as_deref(),
        turns,
    ))];
    let cwd = result["cwd"].as_str().unwrap_or_default();
    for file in result["files"].as_array().into_iter().flatten() {
        lines.extend(file_lines(file, cwd, plan));
    }
    // 执行过几条命令，核心撤销时才交（恢复时不说）。
    if let Some(commands) = result["commands"].as_u64().filter(|commands| *commands > 0) {
        lines.push(Line::gray(language.commands_note(commands, turns)));
    }
    if plan.direction == Direction::Undo {
        lines.push(Line::gray(language.redo_hint()));
    }
    lines
}

/// 一个文件那一行，和它下面的差异。
fn file_lines(file: &Value, cwd: &str, plan: &UndoPlan) -> Vec<Line> {
    let language = &plan.language;
    let action = file["action"].as_str().unwrap_or_default();
    let path = file["path"].as_str().unwrap_or_default();
    let path = cut_front(&clean(&shown(path, cwd, plan.home.as_deref())), PATH_CHARS);
    let mut line = Line::gray(format!("· {} {path}", clean(language.undo_verb(action))));
    match file["outcome"].as_str().unwrap_or_default() {
        "restored" if action == "trash" => {
            line.push(Ink::Gray, format!(" → {}", language.undo_trashed()));
        }
        "restored" => {}
        "failed" => {
            line.push(Ink::Gray, " → ");
            line.push(Ink::Red, language.word(crate::language::Word::Failed));
            if let Some(error) = file["error"].as_str() {
                let error = cut(&clean(error), ERROR_CHARS);
                line.push(Ink::Gray, format!("{}{error}", language.colon()));
            }
        }
        outcome => {
            line.push(Ink::Gray, " → ");
            line.push(Ink::Red, language.untouched());
            if let Some(because) = language.untouched_because(outcome) {
                line.push(Ink::Gray, format!("{}{because}", language.colon()));
            }
        }
    }
    let mut lines = vec![line];
    let diff = file["diff"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default();
    if !diff.is_empty() {
        lines.push(Line::gray(format!(
            "{INDENT}{}",
            language.diff_then(plan.direction)
        )));
        lines.push(Line::gray(format!("{INDENT}{}", language.diff_now())));
        for row in diff.iter().filter_map(Value::as_str) {
            let ink = match row.chars().next() {
                Some('-') => Ink::Red,
                Some('+') => Ink::Green,
                _ => Ink::Gray,
            };
            let mut line = Line::gray(INDENT);
            line.push(ink, keep_tabs(row));
            lines.push(line);
        }
        if let Some(more) = file["more"].as_u64().filter(|more| *more > 0) {
            lines.push(Line::gray(format!("{INDENT}{}", language.diff_more(more))));
        }
    }
    lines
}
