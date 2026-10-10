//! 时间线的一步：标题那一行（`图标 显示名 · 对象`）、收着时的预览、点开以后的全部内容
//! （蓝图 `tui.md`「时间线」第 3–9、11、12、14 条）。

use ratatui::style::Style;
use ratatui::text::Span;
use unicode_width::UnicodeWidthStr;

use super::super::diff_rows;
use super::super::rows::{Ctx, clip};
use super::lit;
use crate::config::ToolKind;
use crate::diff;
use crate::input::{pieces, tail_pieces};
use crate::markdown::{Figure, FigureKind};
use crate::meter;
use crate::theme;
use crate::transcript::{Step, StepKind, ToolState};

/// 一行：引子（预览的 `│ `、`⋮ `，点开内容的缩进，差异的行号）画出来、复制时不带；内容；是不是上一行折下来的。
pub struct Piece {
    /// 引子。
    pub lead: Option<Span<'static>>,
    /// 内容。
    pub content: Vec<Span<'static>>,
    /// 上一行折下来的。
    pub joined: bool,
}

/// 这一步的字的样子：出错的整行红（竖线不跟着红，见 [`rail`]）；别的暗，悬停时变亮。
pub fn style(step: &Step, hovered: bool) -> Style {
    if step.failed() {
        theme::error()
    } else {
        lit(theme::dim(), hovered)
    }
}

/// 标题那一行：图标（出错的换成叉）、显示名、对象、结果那一句；编辑后面跟 `· +3 -3`。`width` 是能写几列，
/// 太长截掉加 `…`。
pub fn title(step: &Step, style: Style, width: u16, ctx: &Ctx) -> Vec<Span<'static>> {
    let text = &ctx.config.text;
    let (label, extra) = match &step.kind {
        StepKind::Thought { .. } => {
            let word = if step.busy() {
                &text.thinking
            } else {
                &text.thought
            };
            (
                format!("{word} · {}", meter::seconds(step.elapsed())),
                Vec::new(),
            )
        }
        StepKind::Tool {
            name,
            state,
            parsed,
            said,
            ..
        } => {
            let face = ctx.human.tool(name);
            let shown = face.map_or(name.as_str(), |f| f.name.as_str());
            if *state == ToolState::Preparing {
                (text.prepare.replace("{name}", shown), Vec::new())
            } else {
                let kind = kind(step, ctx);
                let subject = match kind {
                    Some(ToolKind::Command) => step.arg("description"),
                    _ => face
                        .and_then(|f| f.subject.as_deref())
                        .and_then(|k| step.arg(k)),
                };
                let mut label = shown.to_string();
                // 派子代理：名字后面先写任务编号（`派子代理 · j10 · 描述`）。
                let job = said
                    .as_ref()
                    .filter(|_| kind == Some(ToolKind::Agent))
                    .and_then(|s| s.fields.iter().find(|(f, _)| f.as_str() == "job"))
                    .map(|(_, v)| v.clone());
                if let Some(job) = job {
                    label.push_str(" · ");
                    label.push_str(&job);
                }
                if let Some(subject) = subject {
                    label.push_str(" · ");
                    // 发给别的会话的（C-5，`to` 是会话编号）：写「会话 短编号」，不写整个编号。
                    if crate::session_list::looks_like_session(subject) {
                        let id = crate::session_list::short(subject);
                        label.push_str(&text.on_session.replace("{id}", &id));
                    } else {
                        label.push_str(&crate::local::home_short(subject));
                    }
                }
                // 对着别的会话的（核心 C-4 起 `history` 能翻别的会话）：再跟一格「会话 短编号」（第 8 条）。
                if let Some(session) = step.arg(&ctx.config.timeline.session_arg) {
                    let id = crate::session_list::short(session);
                    label.push_str(" · ");
                    label.push_str(&text.on_session.replace("{id}", &id));
                }
                // 别的工具后面跟结果那一句（「读取 · src · 12 项」）；命令有预览，编辑有加减的行数，不写。
                if matches!(kind, None | Some(ToolKind::Message))
                    && let Some(said) = said
                    && let Some(text) = ctx.human.say(said)
                {
                    label.push_str(" · ");
                    label.push_str(&text);
                }
                let extra = if kind == Some(ToolKind::Edit) {
                    counts(parsed)
                } else {
                    Vec::new()
                };
                (label, extra)
            }
        }
    };
    // 图标取当前那一套（蓝图「图标」）。
    let icons = &ctx.config.icons;
    let icon = match &step.kind {
        _ if step.failed() => &icons.error,
        StepKind::Thought { .. } => &icons.think,
        StepKind::Tool { name, .. } => icons.tool(name),
    };
    let icon = format!("{icon} ");
    let room = usize::from(width)
        .saturating_sub(extra.iter().map(Span::width).sum::<usize>() + icon.width());
    let mut spans = vec![
        Span::styled(icon, style),
        Span::styled(clip(&label, u16::try_from(room).unwrap_or(width)), style),
    ];
    spans.extend(extra);
    spans
}

/// 编辑、写入的 `· +3 -3`：照参数排出来的差异数（和点开以后画的是同一份）。
fn counts(args: &serde_json::Value) -> Vec<Span<'static>> {
    let (added, removed) = diff::from_args(args).map_or((0, 0), |d| (d.added, d.removed));
    if added + removed == 0 {
        return Vec::new();
    }
    vec![
        Span::styled(" · ", theme::dim()),
        Span::styled(format!("+{added}"), theme::added()),
        Span::raw(" "),
        Span::styled(format!("-{removed}"), theme::removed()),
    ]
}

/// 收着时标题下面的预览：思考滚最后几行（想完也留着）；命令预览命令本身，放不下的写「⋮ 已省略 N 行」。
/// 每一行打头是 `│ `，和上下的连接线在同一列（`tui.md`「时间线」第 5、6 条）。
pub fn preview(step: &Step, style: Style, ctx: &Ctx) -> Vec<Piece> {
    let tl = &ctx.config.timeline;
    let width = ctx.width.saturating_sub(2).max(1);
    let bar = |style: Style| Some(Span::styled(format!("{} ", tl.line), style));
    let rail = if step.failed() { style } else { theme::dim() };
    match &step.kind {
        // 思考：滚着显示最后几行，想完也不收起（`tui.md`「时间线」第 5 条）。
        StepKind::Thought { text } => {
            // 还一个字都没有的（刚开始想、只有空白）只有标题，不画空的竖线（第 5 条）。
            if text.trim().is_empty() {
                return Vec::new();
            }
            // 首尾的空行不画：流到一半停在换行上时，不冒出一根空竖线。
            // 只折最后那几行：整段重折的时间跟着思考长度涨，做完的思考也每帧都画。
            tail_pieces(text.trim(), width, tl.thought_rows)
                .into_iter()
                .map(|(l, joined)| Piece {
                    lead: bar(theme::dim()),
                    // 字用主题的 thought（青），和命令的预览分开；悬停时和别的步一起亮一档（第 5 条）。
                    content: vec![Span::styled(l, thought_text(style))],
                    joined,
                })
                .collect()
        }
        StepKind::Tool { .. } if kind(step, ctx) == Some(ToolKind::Command) => {
            // 参数还在流、命令还没有字的，不画预览：不然是一根空的竖线。
            let command = step.arg("command").unwrap_or_default();
            if command.is_empty() {
                return Vec::new();
            }
            let lines = pieces(command, width);
            let mut out: Vec<Piece> = lines
                .iter()
                .take(tl.preview_rows)
                .map(|(l, joined)| Piece {
                    // 竖线不跟着悬停变亮，出错时照旧红（`tui.md`「时间线」第 6、12 条）。
                    lead: bar(rail),
                    content: vec![Span::styled(l.clone(), style)],
                    joined: *joined,
                })
                .collect();
            let more = lines.len().saturating_sub(tl.preview_rows);
            if more > 0 {
                let note = ctx
                    .config
                    .text
                    .omitted
                    .replace("{count}", &more.to_string());
                let mark = Span::styled(format!("{} ", tl.omitted), rail);
                out.push(Piece {
                    lead: Some(mark),
                    content: vec![Span::styled(note, style)],
                    joined: false,
                });
            }
            out
        }
        // 提问答了：一道一行接在这一步的竖线后面（「确认和提问的抽屉」第 6 条）。
        StepKind::Tool { .. } if !step.answers.is_empty() => step
            .answers
            .iter()
            .flat_map(|a| pieces(a, width))
            .map(|(l, joined)| Piece {
                lead: bar(rail),
                content: vec![Span::styled(l, style)],
                joined,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// 点开以后的全部内容：思考是全部的字；命令是命令、空行、输出；编辑、写入是差异，不写结果那一句；别的是输出。
pub fn body(step: &Step, style: Style, width: u16, ctx: &Ctx) -> Vec<Piece> {
    let each = |text: &str, style: Style| {
        pieces(text, width)
            .into_iter()
            .map(move |(l, joined)| Piece {
                lead: None,
                content: vec![Span::styled(l, style)],
                joined,
            })
    };
    match &step.kind {
        StepKind::Thought { text } => each(text.trim(), theme::thought()).collect(),
        // 提问答了：点开也是这几行回答（「确认和提问的抽屉」第 6 条）。
        StepKind::Tool { .. } if !step.answers.is_empty() => {
            step.answers.iter().flat_map(|a| each(a, style)).collect()
        }
        StepKind::Tool { output, parsed, .. } => {
            let diff = (kind(step, ctx) == Some(ToolKind::Edit))
                .then(|| diff::from_args(parsed))
                .flatten();
            if let Some(diff) = diff {
                return diff_rows::lines(&diff, width)
                    .into_iter()
                    .map(|(gutter, content)| Piece {
                        lead: Some(gutter),
                        content: vec![content],
                        joined: false,
                    })
                    .collect();
            }
            // 派子代理：点开是交代的活，结果那一句已经在标题里了。
            if kind(step, ctx) == Some(ToolKind::Agent) {
                return each(step.arg("prompt").unwrap_or_default().trim(), style).collect();
            }
            let mut out = Vec::new();
            if kind(step, ctx) == Some(ToolKind::Command) {
                out.extend(each(step.arg("command").unwrap_or_default(), style));
                out.push(Piece {
                    lead: None,
                    content: Vec::new(),
                    joined: false,
                });
            }
            out.extend(each(output.trim_end(), style));
            out
        }
    }
}

/// 预览左边竖线（和 `⋮`）的样子：出错的步只有字红，竖线照旧暗，和上下的连接线连成一条
/// （`tui.md`「时间线」第 12 条）；不出错的跟着这一步，悬停时一起变亮。
/// 这一步的工具算哪一类；思考、没登记的是 `None`。
fn kind(step: &Step, ctx: &Ctx) -> Option<ToolKind> {
    match &step.kind {
        StepKind::Tool { name, .. } => ctx.config.timeline.kinds.get(name).copied(),
        StepKind::Thought { .. } => None,
    }
}

/// 思考预览的字：平时用主题的 `thought`；悬停（`style` 亮了一档）时用同色系亮一档的 `thought_hover`。
fn thought_text(style: Style) -> Style {
    if style == theme::dim() {
        theme::thought()
    } else {
        theme::thought_hover()
    }
}

/// 结果里带的图（「时间线」第 8 条）：照内容的哈希读回来存成的文件交给画图的那一套；还没读回来的记进单子（和链接卡片
/// 的图同一份账，读回来就重排），读不成的不画。
pub fn figures(step: &Step, ctx: &Ctx) -> Vec<Figure> {
    let mut cards = ctx.cards.borrow_mut();
    step.images
        .iter()
        .filter_map(|blob| cards.file(blob).map(|p| p.display().to_string()))
        .map(|source| Figure {
            kind: FigureKind::Preview,
            source,
            fallback: Vec::new(),
            width: None,
            height: None,
        })
        .collect()
}
