//! 空会话的首页（蓝图 `tui.md`「空会话的首页」）：吉祥物 · 空一行 · 输入框 · 框下面那一行，整组上下居中；
//! 放不下不画吉祥物。

use std::time::Instant;

use ratatui::Frame;
use ratatui::layout::Rect;

use super::Areas;
use crate::app::App;
use crate::config::Layout;

/// 首页各块的位置。`mascot` 是吉祥物占几列几行，`menu_rows` 是列表要露出几行（贴在输入框上面，盖住上面的东西，
/// 输入框不挪）。
pub fn areas(
    area: Rect,
    rows: &dyn Fn(u16) -> u16,
    layout: &Layout,
    mascot: (u16, u16),
    menu_rows: u16,
    agent_rows: u16,
    todo_rows: u16,
) -> Areas {
    // 首页的输入框收窄，和吉祥物成一组（「空会话的首页」第 2 条）。
    let width = super::box_width(area.width, layout).min(layout.home_max_width);
    let x = area.x + (area.width - width) / 2;
    let m = super::margins::margins(layout, area.width);
    let text_width = width.saturating_sub(2 + m.pad_left + m.pad_right).max(1);
    let rows = rows(text_width);
    // 输入框（上下两条边和字）和框下面那一行一直在；待办、吉祥物各连下面的空行。
    let todo_h = if todo_rows > 0 { todo_rows + 1 } else { 0 };
    // 框下面那一行下面：空一行、工作目录（第 2 条）。
    let base = rows + 2 + 1 + 2 + agent_rows + todo_h;
    let (cols, tall) = mascot;
    let mascot_h = tall + 1;
    let show_mascot = cols <= area.width && base + mascot_h <= area.height;
    let total = base + if show_mascot { mascot_h } else { 0 };
    let mut y = area.y + area.height.saturating_sub(total) / 2;
    // 开着列表、输入框上面放不下：整组往下挪，挪到底为止（「窗口小的时候」第 1 条）。
    let over = if show_mascot { mascot_h } else { 0 };
    let lowest = area.bottom().saturating_sub(total);
    y = y.max((area.y + menu_rows.saturating_sub(over)).min(lowest));
    let mascot = if show_mascot {
        let at = Rect::new(area.x + (area.width - cols) / 2, y, cols, tall);
        y += mascot_h;
        at
    } else {
        Rect::new(area.x, y, 0, 0)
    };
    let frame = Rect::new(x, y + todo_h, width, rows + 2).intersection(area);
    let inner_x = frame.x + 1 + m.pad_left;
    let text = Rect::new(inner_x, frame.y + 1, text_width, rows).intersection(frame);
    let footer = Rect::new(inner_x, frame.bottom(), text_width, 1).intersection(area);
    let cwd = Rect::new(inner_x, footer.bottom() + 1, text_width, 1).intersection(area);
    // 列表只用输入框上面剩下的行（「窗口小的时候」第 1 条）。
    let (menu_y, menu_rows) = super::above(frame.y, area.y, menu_rows);
    let menu = Rect::new(inner_x, menu_y, text_width, menu_rows);
    // 待办在列表（没开时是输入框）上面，下面空一行（「后台命令、子代理和侧边栏」第 4 条）。
    let (todo_y, todo_rows) = super::above(menu_y.saturating_sub(1), area.y, todo_rows);
    let todo_y = if todo_rows > 0 { todo_y } else { menu_y };
    let todo = Rect::new(inner_x, todo_y, text_width, todo_rows);
    // 没有正文：高是 0，底边在输入框（开着列表时是列表）上面一行，提示照它浮着（「提示」）。
    let body = Rect::new(frame.x, todo_y.saturating_sub(1), frame.width, 0);
    Areas {
        body,
        menu,
        frame,
        text,
        footer,
        mascot,
        cwd,
        agents: Rect::new(inner_x, cwd.bottom(), text_width, agent_rows).intersection(area),
        todo,
        ..Areas::default()
    }
}

/// 画吉祥物；顺手定吉祥物往哪看（`tui.md`「空会话的首页」第 6、7 条）：输入框里有字看输入光标，
/// 没字看鼠标指针，都没有看正前方。输入框、框下面那一行照平常画，要先画好输入框（光标的位置照它）。
pub fn draw(frame: &mut Frame, areas: Areas, app: &mut App) {
    // 工作目录：暗，放不下从前面截掉（第 2 条）。
    let cwd = tail(&app.cwd, usize::from(areas.cwd.width));
    frame.render_widget(
        ratatui::widgets::Paragraph::new(ratatui::text::Line::styled(cwd, crate::theme::dim())),
        areas.cwd,
    );
    if areas.mascot.height == 0 {
        return;
    }
    let look = &app.config.mascot;
    // 开着列表：被顶到列表上面（中间空一行），关了停一会儿再走回来（第 4 条）。
    // 开着列表时待办也被顶到列表上面，吉祥物再被待办顶上去。
    let top = if areas.todo.height > 0 {
        areas.todo.y
    } else {
        areas.menu.y
    };
    // 抽屉把输入框撑高时也一样顶上去（「确认和提问的抽屉」第 2 条）。
    let pushed = areas.menu.height > 0 || app.drawers.open();
    let needed = pushed.then(|| i32::from(top) - 1 - i32::from(areas.mascot.height));
    let now = Instant::now();
    let Some(y) = app.perch.place(areas.mascot.y, needed, now, &look.perch) else {
        return;
    };
    let areas = Areas {
        mascot: Rect { y, ..areas.mascot },
        ..areas
    };
    super::mascot_view::draw(frame, areas.mascot, app, false);
}

/// 放进 `width` 列：放不下的从前面截掉、打头写 `…`。
fn tail(text: &str, width: usize) -> String {
    use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};
    if text.width() <= width {
        return text.to_string();
    }
    let mut kept: Vec<char> = Vec::new();
    let mut used = 1;
    for c in text.chars().rev() {
        let w = c.width().unwrap_or(0);
        if used + w > width {
            break;
        }
        used += w;
        kept.push(c);
    }
    std::iter::once('…').chain(kept.into_iter().rev()).collect()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use ratatui::layout::Rect;

    use super::areas;
    use crate::config::Config;
    use crate::input::InputBox;

    fn input() -> InputBox {
        InputBox::new(8, Duration::from_millis(400))
    }

    #[test]
    fn the_whole_group_is_centred_vertically() {
        let layout = Config::builtin().unwrap().layout;
        let a = areas(
            Rect::new(0, 0, 100, 40),
            &|w| input().rows(w),
            &layout,
            (36, 16),
            0,
            0,
            0,
        );
        // 吉祥物 16 · 空 · 输入框 3 · 框下面 1 · 空 · 工作目录：共 23 行，上面空 (40 − 23) ÷ 2 = 8 行，多出来的一行放下面。
        assert_eq!((a.mascot.y, a.mascot.height, a.mascot.width), (8, 16, 36));
        assert_eq!(a.mascot.x, (100 - 36) / 2, "吉祥物左右居中");
        assert_eq!(a.frame.y, a.mascot.bottom() + 1, "中间只空一行，不写字");
        assert_eq!(a.frame.height, 3);
        assert_eq!(a.footer.y, a.frame.bottom());
        assert_eq!(a.cwd.y, a.footer.bottom() + 1, "框下面那一行下面空一行");
        assert_eq!((a.cwd.x, a.cwd.height), (a.footer.x, 1), "和框里的字左对齐");
        assert_eq!(40 - a.cwd.bottom(), 9);
    }

    #[test]
    fn the_home_box_is_at_most_home_max_width() {
        let layout = Config::builtin().unwrap().layout;
        let a = areas(
            Rect::new(0, 0, 200, 40),
            &|w| input().rows(w),
            &layout,
            (36, 16),
            0,
            0,
            0,
        );
        assert_eq!(a.frame.width, layout.home_max_width, "宽窗口里收窄");
        assert_eq!(a.frame.x, (200 - layout.home_max_width) / 2, "左右居中");
        let b = areas(
            Rect::new(0, 0, 60, 40),
            &|w| input().rows(w),
            &layout,
            (36, 16),
            0,
            0,
            0,
        );
        assert!(b.frame.width < layout.home_max_width, "窄窗口照平常的宽度");
    }

    #[test]
    fn the_agent_rows_sit_under_the_footer_and_count_in_the_group() {
        let layout = Config::builtin().unwrap().layout;
        let a = areas(
            Rect::new(0, 0, 100, 40),
            &|w| input().rows(w),
            &layout,
            (36, 15),
            0,
            4,
            0,
        );
        assert_eq!(
            (a.agents.y, a.agents.height),
            (a.cwd.bottom(), 4),
            "接在工作目录后面"
        );
        assert_eq!(a.agents.x, a.footer.x, "和框下面那一行的字左对齐");
        // 吉祥物 15 · 空 · 输入框 3 · 框下面 1 · 空 · 工作目录 · 子代理 4：共 26 行。
        assert_eq!(a.mascot.y, (40 - 26) / 2);
    }

    #[test]
    fn the_todo_list_sits_between_the_mascot_and_the_box() {
        let layout = Config::builtin().unwrap().layout;
        let a = areas(
            Rect::new(0, 0, 100, 40),
            &|w| input().rows(w),
            &layout,
            (36, 15),
            0,
            0,
            6,
        );
        // 吉祥物 15 · 空 · 待办 6 · 空 · 输入框 3 · 框下面 1 · 空 · 工作目录：共 29 行。
        assert_eq!(a.mascot.y, (40 - 29) / 2);
        assert_eq!(a.todo.y, a.mascot.bottom() + 1);
        assert_eq!((a.todo.height, a.todo.bottom() + 1), (6, a.frame.y));
        assert_eq!(a.todo.x, a.text.x, "和框里的字左对齐");
        // 开着列表：待办在列表上面，输入框不挪。
        let b = areas(
            Rect::new(0, 0, 100, 40),
            &|w| input().rows(w),
            &layout,
            (36, 15),
            4,
            0,
            6,
        );
        assert_eq!(b.frame, a.frame);
        assert_eq!(b.todo.bottom() + 1, b.menu.y);
    }

    #[test]
    fn a_long_directory_is_cut_from_the_front() {
        assert_eq!(super::tail("~/Documents/github/miyu", 10), "…thub/miyu");
        assert_eq!(super::tail("~/src", 10), "~/src");
    }

    #[test]
    fn a_short_window_drops_the_mascot() {
        let layout = Config::builtin().unwrap().layout;
        let a = areas(
            Rect::new(0, 0, 100, 12),
            &|w| input().rows(w),
            &layout,
            (36, 16),
            0,
            0,
            0,
        );
        assert_eq!(a.mascot.height, 0, "放不下吉祥物");
        assert_eq!(
            a.frame.y, 3,
            "输入框 3 · 框下面 1 · 空 · 工作目录：共 6 行，照样居中"
        );
        assert_eq!(a.footer.y, 6);
        assert_eq!(a.cwd.y, 8);
    }

    #[test]
    fn a_list_leaves_the_box_alone_and_the_mascot_where_it_would_be() {
        let layout = Config::builtin().unwrap().layout;
        let closed = areas(
            Rect::new(0, 0, 100, 40),
            &|w| input().rows(w),
            &layout,
            (36, 15),
            0,
            0,
            0,
        );
        let open = areas(
            Rect::new(0, 0, 100, 40),
            &|w| input().rows(w),
            &layout,
            (36, 15),
            5,
            0,
            0,
        );
        assert_eq!(open.frame, closed.frame, "输入框不挪");
        assert_eq!((open.menu.y, open.menu.height), (closed.frame.y - 5, 5));
        // 吉祥物在哪由 `mascot::Perch` 定（顶上去、停住、走下来），这里给的是不开列表时的位置。
        assert_eq!(open.mascot, closed.mascot);
    }
}
