//! 回到底部的按钮（蓝图 `tui.md`「输入框」，2026-10-02 项目主人要）：正文不在底部时（人滚上去了、长正文替人停在开头），
//! 输入框右边框外面那两列空白里、对着框里写字的第一行，一个 `↓`。暗色，底下有新的字以后强调色；悬停变亮、指针
//! 变手，点了回到底部。窗口窄、框贴着两边没有空白时不画。

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Span;

use crate::app::App;
use crate::theme;

/// 按钮在哪一格：输入框右边框外面空一格的那一格；外面没有两列空白的是 `None`。
pub fn place(frame: Rect, text: Rect, screen: Rect) -> Option<Rect> {
    let x = frame.right().checked_add(1)?;
    (x < screen.right()).then(|| Rect::new(x, text.y, 1, 1))
}

/// 不在底部时画，交回按钮在哪（不画的是空的）。
pub fn draw(frame: &mut Frame, app: &App, screen: Rect) -> Rect {
    let areas = &app.areas;
    let Some(at) = place(areas.frame, areas.text, screen).filter(|_| app.view.top.is_some()) else {
        return Rect::default();
    };
    let hovered = app.pointer.is_some_and(|p| at.contains(p));
    let style = if hovered {
        theme::hover()
    } else if app.view.rows.len() > app.view.seen_end {
        theme::accent()
    } else {
        theme::dim()
    };
    let mark = &app.config.layout.bottom_mark;
    frame.render_widget(Span::styled(mark.clone(), style), at);
    at
}
