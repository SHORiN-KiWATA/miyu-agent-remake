//! 贴在输入框上面的三样共用的样子（蓝图 `tui.md`「斜杠命令列表」第 3 条、「输入历史列表」第 1 条、后台面板）：
//! 圆角框，框线和输入框的边一个颜色（暗），左右和输入框的边对齐；标题嵌在上边框，按键提示嵌在下边框，边框上的字
//! 和框线一个颜色。
//! 一条一行，行首两格；选中的写 `❯ `，整行铺强调色的底、上面的字是深色（`picked_bar`）。放不下框（不到 3 行）的只画
//! 那几条。

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};
use unicode_width::UnicodeWidthStr;

use crate::theme;

/// 面板排好的一行：是第几条（「没有对得上的」这类是 `None`；点开的一条下面几行都算它），和画出来的样子。
pub type Row = (Option<usize>, Line<'static>);

/// 框要几行才画得下：上下两条边，中间至少一条。
const FRAMED: u16 = 3;

/// 框的样子：上边框的标题（标题、条数这些），下边框的按键提示（没有的只是一条线）。
#[derive(Debug, Clone, Default)]
pub struct Chrome {
    /// 嵌在上边框的字。
    pub title: Vec<Span<'static>>,
    /// 嵌在下边框的按键提示。
    pub hint: Option<String>,
}

impl Chrome {
    /// 标题和框线一个颜色，`meta`（条数、搜的字）跟在后面。
    pub fn new(title: &str, meta: Vec<Span<'static>>) -> Self {
        let mut spans = vec![Span::styled(title.to_string(), theme::dim())];
        spans.extend(meta);
        Self {
            title: spans,
            hint: None,
        }
    }

    /// 下边框写这句按键提示。
    pub fn hint(mut self, hint: &str) -> Self {
        self.hint = Some(hint.to_string());
        self
    }
}

/// 给了 `outer` 这么高，框里放得下几条：画得下框的去掉上下两条边。
pub fn room(outer: u16) -> u16 {
    if outer >= FRAMED { outer - 2 } else { outer }
}

/// 放 `rows` 条要多高：连框（放得下的话，照 `outer` 给的最多多高算）。
pub fn height(rows: u16, outer: u16) -> u16 {
    if rows == 0 {
        0
    } else if outer >= FRAMED {
        rows + 2
    } else {
        rows
    }
}

/// 框里放字的那一块：画得下框的去掉上下两条边；左右照输入框里的字（`text_x`、`text_width`）。
pub fn inside(outer: Rect, text_x: u16, text_width: u16) -> Rect {
    let rows = room(outer.height);
    let top = outer.y + (outer.height - rows) / 2;
    Rect::new(text_x, top, text_width, rows)
}

/// 放进 `max` 条：从离选中那一条（`picked`）最远的起少露几条（「窗口小的时候」第 1 条）。
pub fn fit(mut rows: Vec<Row>, picked: Option<usize>, max: usize) -> Vec<Row> {
    while rows.len() > max {
        let anchor = rows
            .iter()
            .position(|(item, _)| item.is_some() && *item == picked)
            .unwrap_or(rows.len());
        let far = rows
            .iter()
            .enumerate()
            .filter(|(_, (item, _))| item.is_some() && *item != picked)
            .max_by_key(|(k, _)| k.abs_diff(anchor))
            .map(|(k, _)| k);
        let Some(far) = far else { break };
        rows.remove(far);
    }
    rows.truncate(max);
    rows
}

/// 画一样：先清掉底下的，画得下框的画框（`outer`），再把几条画进框里（`text`）。
pub fn draw(frame: &mut Frame, outer: Rect, text: Rect, chrome: Chrome, lines: Vec<Line<'static>>) {
    if lines.is_empty() || outer.height == 0 {
        return;
    }
    frame.render_widget(Clear, outer);
    if outer.height >= FRAMED {
        frame.render_widget(block(chrome), outer);
    }
    frame.render_widget(Paragraph::new(lines), text);
}

/// 圆角框：标题嵌在上边框 `╭─ 标题 ─…╮`，按键提示嵌在下边框 `╰─ 提示 ─…╯`。
fn block(chrome: Chrome) -> Block<'static> {
    let line = theme::dim();
    let edge = |mut spans: Vec<Span<'static>>| {
        spans.insert(0, Span::styled("─ ", line));
        spans.push(Span::styled(" ", line));
        Line::from(spans)
    };
    let mut block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(line)
        .title_top(edge(chrome.title));
    if let Some(hint) = chrome.hint {
        block = block.title_bottom(edge(vec![Span::styled(hint, line)]));
    }
    block
}

/// 一条：行首两格（选中的写 `❯ `），`content` 放不下截掉加 `…`，`right` 贴着右边；选中的铺强调色的底、字换深色。
pub fn item(
    picked: bool,
    content: Vec<Span<'static>>,
    right: Option<Span<'static>>,
    width: u16,
) -> Line<'static> {
    let mark = Span::raw(if picked { "❯ " } else { "  " });
    let width = usize::from(width);
    let right_width = right.as_ref().map_or(0, |r| r.width() + 1);
    let room = width.saturating_sub(2 + right_width);
    let mut spans = vec![mark];
    spans.extend(clip_spans(content, room));
    let used: usize = spans.iter().map(Span::width).sum();
    let tail = right.as_ref().map_or(0, Span::width);
    spans.push(Span::raw(" ".repeat(width.saturating_sub(used + tail))));
    spans.extend(right);
    bar(Line::from(spans), picked)
}

/// 接着上一条往下写的一行（展开的全文）：行首空两格，和上一条的字对齐；选中的照 [`item`] 铺底。
pub fn more(picked: bool, content: Vec<Span<'static>>, width: u16) -> Line<'static> {
    let width = usize::from(width);
    let mut spans = vec![Span::raw("  ")];
    spans.extend(clip_spans(content, width.saturating_sub(2)));
    let used: usize = spans.iter().map(Span::width).sum();
    spans.push(Span::raw(" ".repeat(width.saturating_sub(used))));
    bar(Line::from(spans), picked)
}

/// 选中的：整行铺强调色的底，每一段的字换成深色、不要自己的底，加粗这类修饰留着（2026-09-30 项目主人：照 Cline）。
fn bar(line: Line<'static>, picked: bool) -> Line<'static> {
    if !picked {
        return line;
    }
    let dark = theme::picked_bar();
    let spans = line
        .spans
        .into_iter()
        .map(|span| {
            let keep = span.style.add_modifier & (Modifier::BOLD | Modifier::UNDERLINED);
            Span::styled(span.content, dark.add_modifier(keep))
        })
        .collect::<Vec<_>>();
    Line::from(spans).style(dark)
}

/// 这几段排到 `room` 列：放不下的截掉，最后一格写 `…`（样子照被截的那一段）。
pub fn clip_spans(spans: Vec<Span<'static>>, room: usize) -> Vec<Span<'static>> {
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

#[cfg(test)]
mod tests;
