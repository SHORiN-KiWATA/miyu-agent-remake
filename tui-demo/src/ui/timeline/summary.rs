//! 收起那一行（蓝图 `tui.md`「时间线」第 17 条）：永远英文，不带箭头，末尾接这一段的用时。
//!
//! - 只想过：`Thought for 26s`（本来就带时间）；
//! - 做事的只有一条命令（思考不算）、有短标题：`List target project dirs · 1 thought · 3s`；
//! - 别的按类数：`Ran 2 commands · 1 edit · 3 thoughts · 48s`。

use ratatui::style::Style;
use ratatui::text::Span;

use super::super::panel::clip_spans;
use super::super::rows::{Ctx, clip};
use crate::config::ToolKind;
use crate::transcript::StepKind;
use crate::transcript::{Segment, Tally};
use crate::{diff, meter, theme};

/// 收起那一行：照这一段的样子（`base`，出错的整行红）写字；编辑过的那一格后面接加减的总行数，加的绿、减的红
/// （`tui.md`「时间线」第 17 条）。
pub fn line(segment: &Segment, ctx: &Ctx, base: Style) -> Vec<Span<'static>> {
    let words = &ctx.config.text.summary;
    let kind_of = |name: &str| ctx.config.timeline.kinds.get(name).copied();
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
    if tally.commands + tally.tools + tally.edits + tally.agents + tally.messages == 0 {
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
        return clip_title(spans(parts, style, (0, 0)), room(ctx));
    }
    // 打头那一格：命令、子代理、留言、别的工具、编辑，先有哪样写哪样（派子代理的不写 Used 1 tool，2026-10-01 项目主人；
    // 和网页演示一样）；别的类依次跟在后面，打头那一格写过的类不再写。
    let lead = if tally.commands > 0 {
        Lead::Commands
    } else if tally.agents > 0 {
        Lead::Agents
    } else if tally.messages > 0 {
        Lead::Messages
    } else if tally.tools > 0 {
        Lead::Tools
    } else {
        Lead::Edits
    };
    parts.push(match lead {
        Lead::Commands => (count(tally.commands, &words.ran), false),
        Lead::Agents => (count(tally.agents, &words.spawned), false),
        Lead::Messages => (count(tally.messages, &words.messaged), false),
        Lead::Tools => (count(tally.tools, &words.used), false),
        Lead::Edits => (count(tally.edits, &words.made), true),
    });
    if lead != Lead::Edits && tally.edits > 0 {
        parts.push((count(tally.edits, &words.edits), true));
    }
    if lead != Lead::Agents && tally.agents > 0 {
        parts.push((count(tally.agents, &words.agents), false));
    }
    if lead != Lead::Messages && tally.messages > 0 {
        parts.push((count(tally.messages, &words.messages), false));
    }
    if lead != Lead::Tools && tally.tools > 0 {
        parts.push((count(tally.tools, &words.tools), false));
    }
    if tally.thoughts > 0 {
        parts.push((count(tally.thoughts, &words.thoughts), false));
    }
    if tally.errors > 0 {
        parts.push((count(tally.errors, &words.errors), false));
    }
    parts.push((took, false));
    let mut out = spans(parts, style, changed(segment, ctx));
    // 出错的那一格红：整行不红的时候也看得出有一步没成（2026-09-30 项目主人：写入被拒和写成了看起来一样）。
    let errs = count(tally.errors, &words.errors);
    for span in out
        .iter_mut()
        .filter(|s| tally.errors > 0 && s.content == errs)
    {
        span.style = theme::error();
    }
    clip_spans(out, room(ctx))
}

/// 这一行能写几列：去掉行首两格槽。
fn room(ctx: &Ctx) -> usize {
    usize::from(ctx.width).saturating_sub(2)
}

/// 放不下时截短标题（第一段）加 `…`，后面的次数、用时留着；短标题截到一个字都放不下，整行截
/// （`tui.md`「窗口小的时候」第 4 条）。
fn clip_title(mut spans: Vec<Span<'static>>, room: usize) -> Vec<Span<'static>> {
    let total: usize = spans.iter().map(Span::width).sum();
    let Some(first) = spans.first() else {
        return spans;
    };
    if total <= room {
        return spans;
    }
    let rest = total - first.width();
    match room.checked_sub(rest) {
        Some(left) if left >= 2 => {
            let title = clip(&first.content, u16::try_from(left).unwrap_or(u16::MAX));
            spans[0] = Span::styled(title, first.style);
            spans
        }
        _ => clip_spans(spans, room),
    }
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
        // 出错、被拒的编辑没改成：不算加减的行数。
        .filter(|step| !step.failed())
        .filter_map(|step| match &step.kind {
            StepKind::Tool { name, parsed, .. } => {
                let kind = ctx.config.timeline.kinds.get(name).copied();
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

/// 收起那一行打头的是哪一类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lead {
    Commands,
    Agents,
    Messages,
    Tools,
    Edits,
}
