//! 时间线的一段怎么画（`13-终端界面.md` 第三节，项目主人的 TUI 设计稿「AI 输出」一节；蓝图 `tui.md`「时间线」）。
//!
//! - 收起：一行 `Ran 2 commands · 3 thoughts · 48s`，英文，不带箭头（写法在 `summary.rs`）。
//! - 展开：还是那一行，下面一步一行，步与步之间一行连接线 `│`，和图标同一列。进行中的那一段不画收起那一行。
//!
//! 2026-09-29 项目主人试过去掉图标、树状、去掉竖线、英文工具名，样子最后回到这里。
//! - 只有一步的一段：展开就直接铺开这一步的内容，不先列出一行步骤。
//! - 一步的标题、预览、点开的内容在 `step.rs`。

mod step;
mod summary;

use ratatui::style::Style;
use ratatui::text::Span;

use super::rows::{Ctx, Row, Target};
use crate::theme;
use crate::transcript::{Segment, Step};
use step::Piece;

/// 一段排成的行。`entry` 是它在正文里是第几条。
pub fn rows(entry: usize, segment: &Segment, ctx: &Ctx) -> Vec<Row> {
    let target = Target::Segment(entry);
    let mut out = Vec::new();
    let fold = ctx.config.timeline.fold;
    if !segment.expanded(fold) || segment.finished {
        let base = lit(theme::dim(), ctx.hover == Some(target));
        let mut row = ctx.row(ctx.blank_slot(), summary::line(segment, ctx, base));
        row.target = Some(target);
        out.push(row);
        if !segment.expanded(fold) {
            return out;
        }
        // 只有一步的：直接铺开这一步的内容，连收起那一行一起铺底色（`tui.md`「时间线」第 15 条）。
        if let [only] = segment.steps.as_slice() {
            let style = step::style(only, false);
            out.extend(block(step::body(only, style, body_width(ctx), ctx), ctx));
            for row in &mut out {
                row.shade = true;
                row.target = Some(target);
            }
            return out;
        }
    }
    // 同一时刻只转一处（`tui.md`「时间线」第 19 条）。
    let active = segment.active();
    for (j, step) in segment.steps.iter().enumerate() {
        if !out.is_empty() {
            // 上一步出错：竖线一路红到这一步的标题（`tui.md`「时间线」第 12 条）。
            let failed = j > 0 && segment.steps[j - 1].failed();
            out.push(connector(ctx, failed));
        }
        out.extend(step_rows(
            Target::Step(entry, j),
            step,
            active == Some(j),
            ctx,
        ));
    }
    out
}

/// 步与步之间那一行 `│`，和图标同一列；上一步出错的红。
fn connector(ctx: &Ctx, failed: bool) -> Row {
    let style = if failed { theme::error() } else { theme::dim() };
    ctx.row(
        ctx.blank_slot(),
        vec![Span::styled(ctx.config.timeline.line.clone(), style)],
    )
}

/// 一步的行：标题，下面接着预览或点开的内容。`spinning` 是这一步在转圈（这一段正在动的那一步）。
fn step_rows(target: Target, step: &Step, spinning: bool, ctx: &Ctx) -> Vec<Row> {
    let tl = &ctx.config.timeline;
    let style = step::style(step, ctx.hover == Some(target));
    let slot = if spinning {
        let frame = &tl.spinner[ctx.frame % tl.spinner.len().max(1)];
        Span::styled(format!("{frame} "), theme::dim())
    } else {
        ctx.blank_slot()
    };
    let mut head = ctx.row(slot, step::title(step, style, ctx.width, ctx));
    head.target = Some(target);
    let mut out = vec![head];
    if step.opened(&ctx.config.timeline) {
        out.extend(block(step::body(step, style, body_width(ctx), ctx), ctx));
        for row in &mut out {
            row.shade = true;
        }
    } else {
        out.extend(
            step::preview(step, style, ctx)
                .into_iter()
                .map(|piece| line(piece, ctx)),
        );
    }
    for row in &mut out {
        row.target = Some(target);
    }
    out
}

/// 点开的内容：空行、全部内容、空行；内容缩进两格，和图标后面的字对齐。
fn block(pieces: Vec<Piece>, ctx: &Ctx) -> Vec<Row> {
    let blank = || Piece {
        lead: None,
        content: Vec::new(),
        joined: false,
    };
    std::iter::once(blank())
        .chain(pieces)
        .chain(std::iter::once(blank()))
        .map(|piece| {
            let mut lead = vec![Span::raw(INDENT)];
            lead.extend(piece.lead);
            let mut row = ctx.led_row(ctx.blank_slot(), lead, piece.content);
            row.joined = piece.joined;
            row
        })
        .collect()
}

/// 点开的内容缩进几格：和图标后面的字对齐。
const INDENT: &str = "  ";

/// 点开的内容能写几列。
fn body_width(ctx: &Ctx) -> u16 {
    ctx.width.saturating_sub(2).max(1)
}

/// 一行：引子（预览的 `│ `、`⋮ `）画出来、复制时不带，再接内容。
fn line(piece: Piece, ctx: &Ctx) -> Row {
    let mut row = ctx.led_row(
        ctx.blank_slot(),
        piece.lead.into_iter().collect(),
        piece.content,
    );
    row.joined = piece.joined;
    row
}

/// 悬停时从暗变亮（设计稿：鼠标悬浮时颜色稍微亮起）。
fn lit(style: Style, hovered: bool) -> Style {
    if hovered { theme::hover() } else { style }
}

#[cfg(test)]
mod tests;
