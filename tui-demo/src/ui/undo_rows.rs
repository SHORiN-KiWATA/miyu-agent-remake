//! 撤销说明那一行（蓝图 `tui.md`「正文」第 5 条）：收着一行 `↶ 已撤销 · 改回 N 个文件 · /restore 恢复 · 那一句的预览`；
//! 点开铺底色：你说的那句全文，改回的文件一个一行（改回了的 `✓`、没动的 `✗` 写为什么），有差异的文件再点一下展开
//! 差异（和时间线里点开一步的 diff 一个样子），最后一行停掉了几个任务。

use ratatui::text::Span;
use unicode_width::UnicodeWidthStr;

use super::diff_rows;
use super::rows::{Ctx, Row, Target, clip};
use crate::core::UndoFile;
use crate::diff::unified;
use crate::input::pieces;
use crate::theme;
use crate::transcript::{Entry, undo_counts};

/// 撤销说明那一条的几行。
pub(super) fn rows(i: usize, entry: &Entry, ctx: &Ctx) -> Vec<Row> {
    let target = Target::Entry(i);
    let style = if ctx.hover == Some(target) {
        theme::hover()
    } else {
        theme::dim()
    };
    let text = &ctx.config.text;
    let report = entry.undo.as_ref();
    let mut head = format!("{}{}", ctx.config.layout.undo_icon, text.undone);
    let files = merged(report.map(|r| r.files.as_slice()).unwrap_or_default());
    let restored = files.iter().filter(|f| f.restored()).count();
    if restored > 0 {
        head.push_str(" · ");
        head.push_str(&text.restored.replace("{count}", &restored.to_string()));
    }
    head.push_str(" · ");
    head.push_str(&text.undo_restore);
    let peek: String = entry.text.split_whitespace().collect::<Vec<_>>().join(" ");
    if !peek.is_empty() {
        head.push_str(" · ");
        head.push_str(&peek);
    }
    let mut out = vec![ctx.row(
        ctx.blank_slot(),
        vec![Span::styled(clip(&head, ctx.width), style)],
    )];
    // 撤掉的几轮里有压缩、有清空：下面一行说一句，和「已撤销」对齐（施工 6-9、6-8 补，照 `miyu undo`）。
    for (count, said) in [
        (report.map_or(0, |r| r.compactions), &text.undo_compactions),
        (report.map_or(0, |r| r.clears), &text.undo_clears),
    ] {
        if count == 0 {
            continue;
        }
        let indent = " ".repeat(ctx.config.layout.undo_icon.width());
        out.push(ctx.led_row(
            ctx.blank_slot(),
            vec![Span::raw(indent)],
            vec![Span::styled(said.clone(), style)],
        ));
    }
    let mut details = Vec::new();
    if entry.open {
        let width = ctx.width.saturating_sub(2).max(1);
        let blank = || ctx.row(ctx.blank_slot(), Vec::new());
        out.push(blank());
        for (piece, joined) in pieces(entry.text.trim(), width) {
            let mut row = ctx.led_row(
                ctx.blank_slot(),
                vec![Span::raw("  ")],
                vec![Span::raw(piece)],
            );
            row.joined = joined;
            out.push(row);
        }
        if !files.is_empty() {
            out.push(blank());
        }
        // 补发来的没有工作目录（回应里才有）：照界面所在的。
        let here = std::env::current_dir().ok();
        let cwd = report
            .and_then(|r| r.cwd.clone())
            .or_else(|| here.map(|h| h.to_string_lossy().into_owned()));
        let cwd = cwd.as_deref();
        for (k, file) in files.iter().enumerate() {
            let opened = entry.details.contains(&k);
            let mut file_rows = file_lines(file, cwd, opened, ctx);
            if !file.diff.is_empty() {
                details.push((out.len(), out.len() + file_rows.len(), k));
            }
            out.append(&mut file_rows);
        }
        if let Some(counts) = report.and_then(|r| undo_counts(r, text)) {
            out.push(blank());
            out.push(ctx.led_row(
                ctx.blank_slot(),
                vec![Span::raw("  ")],
                vec![Span::styled(counts, theme::dim())],
            ));
        }
        out.push(blank());
        for row in &mut out {
            row.shade = true;
        }
    }
    for row in &mut out {
        row.target = Some(target);
    }
    // 有差异的文件那几行点了是展开、收起它的差异。
    for (from, to, k) in details {
        for row in &mut out[from..to] {
            row.target = Some(Target::Details(i, k));
        }
    }
    out
}

/// 一个文件：`✓ src/a.rs  已改回  +4 −2`、`✗ README.md  没动：之后又被改过`；`opened` 的接着差异。
fn file_lines(file: &UndoFile, cwd: Option<&str>, opened: bool, ctx: &Ctx) -> Vec<Row> {
    let layout = &ctx.config.layout;
    let words = &ctx.config.text.undo_files;
    let (mark, mark_style) = if file.restored() {
        (&layout.undo_restored_mark, theme::good())
    } else {
        (&layout.undo_left_mark, theme::warn())
    };
    let mut said = words.outcome(&file.outcome).to_string();
    if let Some(error) = file.error.as_deref().filter(|_| file.outcome == "failed") {
        said.push('：');
        said.push_str(error);
    }
    let diff = (!file.diff.is_empty()).then(|| unified::read(&file.diff));
    let added = file.added.or(diff.as_ref().map(|d| d.added as u64));
    let removed = file.removed.or(diff.as_ref().map(|d| d.removed as u64));
    let mut content = vec![
        Span::styled(mark.clone(), mark_style),
        Span::raw(short(&file.path, cwd)),
        Span::styled(format!("  {said}"), theme::dim()),
    ];
    if let Some(added) = added.filter(|&n| n > 0) {
        content.push(Span::styled(format!("  +{added}"), theme::diff_added()));
    }
    if let Some(removed) = removed.filter(|&n| n > 0) {
        content.push(Span::styled(format!(" −{removed}"), theme::diff_removed()));
    }
    let mut out = vec![ctx.led_row(ctx.blank_slot(), vec![Span::raw("  ")], content)];
    if let Some(diff) = diff.filter(|_| opened) {
        let width = ctx.width.saturating_sub(6).max(1);
        for (gutter, line) in diff_rows::lines(&diff, width) {
            out.push(ctx.led_row(
                ctx.blank_slot(),
                vec![Span::raw("    "), gutter],
                vec![line],
            ));
        }
        if file.more > 0 {
            let more = words.more.replace("{count}", &file.more.to_string());
            out.push(ctx.led_row(
                ctx.blank_slot(),
                vec![Span::raw("    ")],
                vec![Span::styled(more, theme::dim())],
            ));
        }
    }
    out
}

/// 同一个文件改回了好几步（先写、再编辑）：并成一行。都改回了才算改回了，不然照第一个没动的写；差异接起来，
/// 加减的行数相加。
fn merged(files: &[UndoFile]) -> Vec<UndoFile> {
    let mut out: Vec<UndoFile> = Vec::new();
    for file in files {
        let Some(kept) = out.iter_mut().find(|f| f.path == file.path) else {
            out.push(file.clone());
            continue;
        };
        if kept.restored() && !file.restored() {
            kept.outcome.clone_from(&file.outcome);
            kept.error.clone_from(&file.error);
        }
        kept.diff.extend(file.diff.iter().cloned());
        kept.more += file.more;
        let sum = |a: Option<u64>, b: Option<u64>| match (a, b) {
            (None, None) => None,
            (a, b) => Some(a.unwrap_or(0) + b.unwrap_or(0)),
        };
        kept.added = sum(kept.added, file.added);
        kept.removed = sum(kept.removed, file.removed);
    }
    out
}

/// 路径写短：在工作目录里的写相对的，家目录写成 `~`。
fn short(path: &str, cwd: Option<&str>) -> String {
    if let Some(rest) = cwd
        .and_then(|c| path.strip_prefix(c))
        .and_then(|r| r.strip_prefix('/'))
    {
        return rest.to_string();
    }
    crate::local::home_short(path)
}
