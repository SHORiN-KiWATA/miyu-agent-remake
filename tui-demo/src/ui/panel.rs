//! 贴在输入框上面的面板共用的样子（蓝图 `tui.md`「输入历史列表」第 1 条、「斜杠命令列表」第 3 条、后台面板）：
//! 标题写在强调色的横线上 `── 标题 · 条数 ────`，空一行；一条一行，选中的写 `❯ `、整行铺底色，右边一截暗色的字
//! 贴着右边；空一行，最后一行暗色的按键提示。斜杠命令列表只用标题横线和一条一行（每打一个 `/` 都弹，不要太高）。

use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use crate::theme;

/// 顶上的两行：标题横线、空行。
pub fn head(title: &str, meta: Vec<Span<'static>>, width: u16) -> Vec<Line<'static>> {
    vec![rule(title, meta, width), Line::raw("")]
}

/// 标题写在横线上：`── ` 标题（强调色加粗）、`meta`、一格空，横线铺到 `width` 列（2026-09-29 项目主人：标题都写在横线上）。
pub fn rule(title: &str, meta: Vec<Span<'static>>, width: u16) -> Line<'static> {
    let accent = theme::accent();
    let mut spans = vec![
        Span::styled("── ", accent),
        Span::styled(title.to_string(), accent.add_modifier(Modifier::BOLD)),
    ];
    spans.extend(meta);
    spans.push(Span::raw(" "));
    let used: usize = spans.iter().map(Span::width).sum();
    spans.push(Span::styled(
        "─".repeat(usize::from(width).saturating_sub(used)),
        accent,
    ));
    Line::from(spans)
}

/// 最后两行：空行、暗色的按键提示。
pub fn hints(text: &str) -> Vec<Line<'static>> {
    vec![Line::raw(""), Line::styled(text.to_string(), theme::dim())]
}

/// 一条：行首两格（选中的写 `❯ `），`content` 放不下截掉加 `…`，`right` 贴着右边；选中的整行铺底色。
pub fn item(
    picked: bool,
    content: Vec<Span<'static>>,
    right: Option<Span<'static>>,
    width: u16,
) -> Line<'static> {
    let mark = if picked {
        Span::styled("❯ ", theme::picked())
    } else {
        Span::raw("  ")
    };
    let width = usize::from(width);
    let right_width = right.as_ref().map_or(0, |r| r.width() + 1);
    let room = width.saturating_sub(2 + right_width);
    let mut spans = vec![mark];
    spans.extend(clip_spans(content, room));
    let used: usize = spans.iter().map(Span::width).sum();
    let tail = right.as_ref().map_or(0, Span::width);
    spans.push(Span::raw(" ".repeat(width.saturating_sub(used + tail))));
    spans.extend(right);
    shaded(Line::from(spans), picked)
}

/// 接着上一条往下写的一行（展开的全文）：行首空两格，和上一条的字对齐；选中的铺底色。
pub fn more(picked: bool, content: Vec<Span<'static>>, width: u16) -> Line<'static> {
    let width = usize::from(width);
    let mut spans = vec![Span::raw("  ")];
    spans.extend(clip_spans(content, width.saturating_sub(2)));
    let used: usize = spans.iter().map(Span::width).sum();
    spans.push(Span::raw(" ".repeat(width.saturating_sub(used))));
    shaded(Line::from(spans), picked)
}

/// 选中的整行铺底色。
fn shaded(line: Line<'static>, picked: bool) -> Line<'static> {
    if picked {
        line.style(theme::shade())
    } else {
        line
    }
}

/// 这几段排到 `room` 列：放不下的截掉，最后一格写 `…`（样子照被截的那一段）。
fn clip_spans(spans: Vec<Span<'static>>, room: usize) -> Vec<Span<'static>> {
    let total: usize = spans.iter().map(Span::width).sum();
    if total <= room {
        return spans;
    }
    let mut out = Vec::new();
    let mut used = 0;
    for span in spans {
        let mut kept = String::new();
        for c in span.content.chars() {
            let w = c.to_string().width();
            if used + w + 1 > room {
                break;
            }
            kept.push(c);
            used += w;
        }
        let cut = kept.len() < span.content.len();
        if !kept.is_empty() {
            out.push(Span::styled(kept, span.style));
        }
        if cut {
            if room > 0 {
                out.push(Span::styled("…", span.style));
            }
            return out;
        }
    }
    out
}
