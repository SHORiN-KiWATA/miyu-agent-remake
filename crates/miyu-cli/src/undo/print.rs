//! 照核心交回的几样写成一行行给人看的（样子是项目主人 2026-09-28 定的，`docs/construction/4-7-miyu undo、miyu
//! redo（下）.md`）：第一行说撤的是哪一轮，撤掉了压缩的说一句上下文回到了压缩前（施工 6-9），撤掉了清空的说一句回到了
//! 清空以前（施工 6-8 补），停掉了那几轮派出去的任务的说一句停掉了几个（施工 7-8），每个文件一行，没动的同一行
//! 写原因，之后又被改过的下面印差异，执行过命令的说一句撤不回，撤销的最后说怎么恢复。只管写成什么样，不管往哪写。
//!
//! `miyu redo` 也照这几行印撤掉了哪一轮（施工 4-7 再补，`docs/blueprint/cli/redo.md`）：第一行接「，重新做」，不说怎么恢复。

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
    let header = plan
        .language
        .undo_header(plan.direction, said(result).as_deref(), turns(result));
    let mut lines = vec![Line::gray(header)];
    lines.extend(body(result, plan));
    if plan.direction == Direction::Undo {
        lines.push(Line::gray(plan.language.restore_hint()));
    }
    lines
}

/// 重做的回应写成的几行（施工 4-7 再补）：第一行照撤销那一行接「，重新做」，接着照撤销印，不说怎么恢复：重做以后恢复不了。
/// `plan` 照撤销的写（[`Direction::Undo`]）：差异的头一行对照的是她改完的。
pub(crate) fn redo_lines(result: &Value, plan: &UndoPlan) -> Vec<Line> {
    let mut lines = vec![Line::gray(
        plan.language.redo_header(said(result).as_deref()),
    )];
    lines.extend(body(result, plan));
    lines
}

/// 那一轮人说的话：控制字符换掉，太长的截掉。
fn said(result: &Value) -> Option<String> {
    result["said"]
        .as_str()
        .map(|said| cut(&clean(said), SAID_CHARS))
}

/// 撤了几轮，回应里没有的当 1。
fn turns(result: &Value) -> usize {
    result["turns"]
        .as_u64()
        .and_then(|turns| usize::try_from(turns).ok())
        .unwrap_or(1)
}

/// 第一行和最后一行中间的：撤掉了压缩、清空的那两句，停掉了几个任务的那一句，每个文件和差异，执行过命令的那一句。
fn body(result: &Value, plan: &UndoPlan) -> Vec<Line> {
    let language = &plan.language;
    let turns = turns(result);
    let mut lines = Vec::new();
    // 撤掉了几次压缩、几次清空，核心撤销时才交，是 0 的不交：几次都说同一句，两样都有的先压缩后清空；恢复不说（施工 6-9、
    // 6-8 补）。
    let undone = |field: &str| result[field].as_u64().is_some_and(|count| count > 0);
    if plan.direction == Direction::Undo {
        if undone("compactions") {
            lines.push(Line::gray(language.compaction_undone()));
        }
        if undone("clears") {
            lines.push(Line::gray(language.clear_undone()));
        }
        // 停掉了几个任务，核心撤销时才交，没有的不交（施工 7-8）：只说几个，是哪几个在回应里。
        let stopped = result["jobs"].as_array().map_or(0, Vec::len);
        if stopped > 0 {
            lines.push(Line::gray(language.jobs_stopped(stopped)));
        }
    }
    let cwd = result["cwd"].as_str().unwrap_or_default();
    for file in result["files"].as_array().into_iter().flatten() {
        lines.extend(file_lines(file, cwd, plan));
    }
    // 执行过几条命令，核心撤销时才交（恢复时不说）。
    if let Some(commands) = result["commands"].as_u64().filter(|commands| *commands > 0) {
        lines.push(Line::gray(language.commands_note(commands, turns)));
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
