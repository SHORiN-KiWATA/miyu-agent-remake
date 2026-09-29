//! 收起那一行（蓝图 `tui.md`「时间线」第 17 条）：永远英文，不带箭头，末尾接这一段的用时。
//!
//! - 只想过：`Thought for 26s`（本来就带时间）；
//! - 做事的只有一条命令（思考不算）、有短标题：`List target project dirs · 1 thought · 3s`；
//! - 别的按类数：`Ran 2 commands · 1 edit · 3 thoughts · 48s`。

use ratatui::style::Style;
use ratatui::text::Span;

use super::super::rows::Ctx;
use crate::config::ToolKind;
use crate::transcript::StepKind;
use crate::transcript::{Segment, Tally};
use crate::{diff, meter, theme};

/// 收起那一行：照这一段的样子（`base`，出错的整行红）写字；编辑过的那一格后面接加减的总行数，加的绿、减的红
/// （`tui.md`「时间线」第 17 条）。
pub fn line(segment: &Segment, ctx: &Ctx, base: Style) -> Vec<Span<'static>> {
    let words = &ctx.config.text.summary;
    let kind_of = |name: &str| ctx.config.timeline.tools.get(name).and_then(|t| t.kind);
    let tally = Tally::count(segment, kind_of);
    let style = if failed(segment, &tally) {
        theme::error()
    } else {
        base
    };
    let count = |n: usize, forms: &[String; 2]| {
        forms[usize::from(n != 1)].replace("{count}", &n.to_string())
    };
    // 用时：四舍五入到整秒，不到一秒的写 1s，照运行状态行的读秒写。
    let took = meter::clock(((tally.span.as_millis() + 500) / 1000).max(1) as u64);
    if tally.commands + tally.tools + tally.edits == 0 {
        let secs = format!("{}s", tally.thinking.as_secs().max(1));
        return vec![Span::styled(
            words.thought_for.replace("{elapsed}", &secs),
            style,
        )];
    }
    // 一格一格：字，和这一格要不要接加减的行数。
    let mut parts: Vec<(String, bool)> = Vec::new();
    if let Some(title) = &tally.only_command {
        parts.push((title.clone(), false));
        if tally.thoughts > 0 {
            parts.push((count(tally.thoughts, &words.thoughts), false));
        }
        if tally.errors > 0 {
            parts.push((count(tally.errors, &words.errors), false));
        }
        parts.push((took, false));
        return spans(parts, style, (0, 0));
    }
    let lead = [
        (tally.commands, &words.ran),
        (tally.tools, &words.used),
        (tally.edits, &words.made),
    ]
    .into_iter()
    .position(|(n, _)| n > 0);
    match lead {
        Some(0) => parts.push((count(tally.commands, &words.ran), false)),
        Some(1) => parts.push((count(tally.tools, &words.used), false)),
        _ => parts.push((count(tally.edits, &words.made), true)),
    }
    // 打头那一格已经写过的类不再写一遍。
    if lead != Some(2) && tally.edits > 0 {
        parts.push((count(tally.edits, &words.edits), true));
    }
    if lead == Some(0) && tally.tools > 0 {
        parts.push((count(tally.tools, &words.tools), false));
    }
    if tally.thoughts > 0 {
        parts.push((count(tally.thoughts, &words.thoughts), false));
    }
    if tally.errors > 0 {
        parts.push((count(tally.errors, &words.errors), false));
    }
    parts.push((took, false));
    spans(parts, style, changed(segment, ctx))
}

/// 只有一条命令、它出错了：整行红。
fn failed(segment: &Segment, tally: &Tally) -> bool {
    tally.only_command.is_some() && tally.errors > 0 && !segment.steps.is_empty()
}

/// 这一段的编辑、写入一共加了几行、删了几行（照参数排出来的差异）。
fn changed(segment: &Segment, ctx: &Ctx) -> (usize, usize) {
    segment
        .steps
        .iter()
        .filter_map(|step| match &step.kind {
            StepKind::Tool { name, parsed, .. } => {
                let kind = ctx.config.timeline.tools.get(name).and_then(|t| t.kind);
                (kind == Some(ToolKind::Edit))
                    .then(|| diff::from_args(parsed))
                    .flatten()
            }
            StepKind::Thought { .. } => None,
        })
        .fold((0, 0), |(a, r), d| (a + d.added, r + d.removed))
}

/// 一格一格用 ` · ` 接起来；要接行数的那一格后面接 ` +3 -1`（都是 0 的不接）。
fn spans(
    parts: Vec<(String, bool)>,
    style: Style,
    (added, removed): (usize, usize),
) -> Vec<Span<'static>> {
    let mut out = Vec::new();
    for (i, (text, counts)) in parts.into_iter().enumerate() {
        if i > 0 {
            out.push(Span::styled(" · ", style));
        }
        out.push(Span::styled(text, style));
        if counts && added + removed > 0 {
            out.push(Span::styled(" ", style));
            out.push(Span::styled(format!("+{added}"), theme::added()));
            out.push(Span::styled(" ", style));
            out.push(Span::styled(format!("-{removed}"), theme::removed()));
        }
    }
    out
}
