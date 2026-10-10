//! 浮在一步上面的窗（蓝图 `tui.md`「第一次打开的引导」第 21a 条）：后面调暗，盖一块浅一档的底，最上面一行标题（强调色
//! 加粗），中间照给的一块画（比窗高的照光标滚），最下面一行暗色按键提示。不画边框（同配置页的悬浮窗）。

use ratatui::Frame;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::Paragraph;

use super::content::Content;
use crate::oobe::look::Popup as Look;
use crate::theme;

/// 窗在哪：照 `oobe.json` 占窗口的几成、最宽几列，居中。
pub fn rect(area: Rect, look: &Look) -> Rect {
    let part = |whole: u16, share: f64| (f64::from(whole) * share).round() as u16;
    let width = part(area.width, look.width)
        .min(look.max_width)
        .min(area.width.saturating_sub(2));
    let height = part(area.height, look.height).min(area.height.saturating_sub(2));
    Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    )
}

/// 中间写字的那一块：左右各空两列，标题、按键提示各占一行，和中间各隔一行。
pub fn body(rect: Rect) -> Rect {
    Rect::new(
        rect.x + 2,
        rect.y + 3,
        rect.width.saturating_sub(4),
        rect.height.saturating_sub(6),
    )
}

/// 画一个窗：交回输入光标落在哪（有的话）。
pub fn draw(
    frame: &mut Frame,
    area: Rect,
    rect: Rect,
    (title, content, hint): (&str, &Content, &str),
) -> Option<Position> {
    let buf = frame.buffer_mut();
    buf.set_style(area, Style::new().add_modifier(Modifier::DIM));
    // 先把格子整个清掉（后面调暗的修饰不留到窗里），再铺底色。
    for y in rect.y..rect.bottom() {
        for x in rect.x..rect.right() {
            if let Some(cell) = buf.cell_mut(Position::new(x, y)) {
                cell.reset();
            }
        }
    }
    buf.set_style(rect, theme::panel());
    let inner = body(rect);
    let at_row = |y: u16| Rect::new(inner.x, y, inner.width, 1);
    let title = Line::styled(
        title.to_string(),
        theme::accent().add_modifier(Modifier::BOLD),
    );
    frame.render_widget(Paragraph::new(title), at_row(rect.y + 1));
    let hint = Line::styled(hint.to_string(), theme::faint());
    frame.render_widget(
        Paragraph::new(hint),
        at_row(rect.bottom().saturating_sub(2)),
    );
    let scroll = super::place(frame, inner, content, 0);
    let (row, col) = content.caret?;
    let at = Position::new(inner.x + col, inner.y + row.checked_sub(scroll)?);
    inner.contains(at).then_some(at)
}
