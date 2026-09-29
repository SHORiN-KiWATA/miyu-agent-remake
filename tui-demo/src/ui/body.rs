//! 正文区：把排好的行放进视口，铺上点开的底色和选区的反色。
//!
//! 放得下时从顶上开始；放不下时露出最新的那一截，滚过以后钉在滚到的地方（`body_view.rs`）。

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use super::Areas;
use super::row_cache::{self, Rows};
use super::rows::Ctx;
use crate::app::App;
use crate::theme;

/// 画正文，顺手把这一帧的行记进 `app.view`，鼠标要用。
pub fn draw(frame: &mut Frame, areas: Areas, app: &mut App) {
    let area = areas.body;
    let spinner_ms = app.config.timeline.spinner_ms.max(1);
    let ctx = Ctx {
        config: &app.config,
        human: &app.human,
        // 两格槽紧贴在输入框里的字的左边，和提示符同一列。
        indent: " ".repeat(usize::from(areas.text.x.saturating_sub(area.x + 2))),
        width: areas.text.width,
        hover: app.view.hover,
        level: app.transcript.level,
        md: &app.md_cache,
        figures: &app.figures,
        frame: usize::try_from(app.started.elapsed().as_millis() / u128::from(spinner_ms))
            .unwrap_or(0),
    };
    let rows = row_cache::build(&app.transcript.entries, &ctx, &app.row_cache);
    let first = first_row(&rows, area, &mut app.view);
    let height = usize::from(area.height);
    for (i, row) in rows.window(first, height) {
        let y = area.y + u16::try_from(i - first).unwrap_or(0);
        let line_area = Rect::new(area.x, y, area.width, 1);
        // 先铺底色再写字：没带底色的片段留着底下的灰，差异行自己的红底、青底盖在上面。
        if row.shade {
            frame.buffer_mut().set_style(line_area, theme::shade());
        }
        frame
            .buffer_mut()
            .set_line(area.x, y, &row.line, area.width);
    }
    app.view.first = first;
    super::figure_rows::draw(
        frame.buffer_mut(),
        area,
        &rows,
        first,
        &app.figures.borrow(),
    );
    // 鼠标、复制照这一帧的行；共享记着的那一份，不复制。
    app.view.rows = rows.clone();
    // 悬停在链接上：这个链接露出来的每一截都加下划线。
    if let Some(url) = &app.view.hover_link {
        for (i, row) in rows.window(first, height) {
            let y = area.y + u16::try_from(i - first).unwrap_or(0);
            for (from, to, _) in row.links.iter().filter(|(_, _, u)| u == url) {
                let x = area.x + row.content_x + from;
                let cells = Rect::new(x, y, to - from, 1).intersection(area);
                let underline = Style::new().add_modifier(Modifier::UNDERLINED);
                frame.buffer_mut().set_style(cells, underline);
            }
        }
    }
    for (i, row) in rows.window(first, height) {
        // 图的行不铺反色：kitty 的图靠格子的前景色认是哪张图，反了就画不出来。
        if row.figure.is_some() {
            continue;
        }
        if let Some((from, to)) = app.view.selected_cols(i) {
            let y = area.y + u16::try_from(i - first).unwrap_or(0);
            let cols = Rect::new(area.x + from, y, to - from, 1).intersection(area);
            let reversed = Style::new().add_modifier(Modifier::REVERSED);
            frame.buffer_mut().set_style(cols, reversed);
        }
    }
}

/// 第一行露出的是第几行：有锚点的照锚点，滚过的照滚到的，别的跟着最新的、只往下走。滚到底了就回到跟着最新的。
/// 顺手记下这一帧的区域（`view.area`），下一帧照它看窗口宽度变没变。
fn first_row(rows: &Rows, area: Rect, view: &mut crate::body_view::BodyView) -> usize {
    let height = usize::from(area.height);
    let bottom = rows.len().saturating_sub(height);
    if let Some((target, y)) = view.anchor.take()
        && let Some(i) = rows.iter().position(|r| r.target == Some(target))
    {
        view.top = Some(i.saturating_sub(usize::from(y.saturating_sub(area.y))));
    }
    // 窗口变宽变窄，行重新折过，上一帧记的行号对不上了：不守它（`tui.md`「正文」第 1 条）。
    if view.area.width != area.width {
        view.floor_end = 0;
    }
    // 清过屏的，最底下是清屏那一刻的位置：往下滚还是空的，滚回底又是空的（`tui.md`「按键」Ctrl+L）。
    let lowest = view
        .cleared_at
        .map_or(bottom, |at| bottom.max(at.min(rows.len())));
    let first = match view.top {
        Some(top) => top.min(lowest),
        // 跟着最新的：不比上一帧靠上（蓝图 tui.md「正文」第 1 条），但总要露出至少一行；清过屏的可以一行都不露。
        None => {
            let cap = if view.cleared_at.is_some() {
                rows.len()
            } else {
                rows.len().saturating_sub(1)
            };
            lowest.max(view.floor_end.saturating_sub(height)).min(cap)
        }
    };
    if first >= lowest {
        view.top = None;
    }
    // 内容还放得下（从第一行露起；清过屏的，从清屏那一行露起）时不记：视口变矮时不把开头的行挤出去
    // （`tui.md`「正文」第 1 条、「按键」Ctrl+L）。
    let top = view.cleared_at.map_or(0, |at| at.min(rows.len()));
    view.floor_end = if first == top { 0 } else { first + height };
    view.area = area;
    first
}

#[cfg(test)]
mod tests {
    use ratatui::layout::Rect;
    use ratatui::text::Line;

    use super::first_row;
    use crate::body_view::BodyView;
    use crate::ui::rows::Row;

    fn rows(n: usize) -> crate::ui::row_cache::Rows {
        let row = Row {
            line: Line::default(),
            target: None,
            shade: false,
            plain: String::new(),
            content_x: 0,
            joined: false,
            links: Vec::new(),
            copy: true,
            figure: None,
        };
        vec![row; n].into()
    }

    #[test]
    fn shrinking_content_does_not_pull_the_rows_back_down() {
        let area = Rect::new(0, 0, 10, 10);
        let mut view = BodyView::default();
        // 30 行露 10 行：跟着最新的，第一行是第 20 行。
        assert_eq!(first_row(&rows(30), area, &mut view), 20);
        // 思考的预览收起，少了 8 行：已经顶上去的不掉下来。
        assert_eq!(first_row(&rows(22), area, &mut view), 20);
        // 新的字来了，接着往下走。
        assert_eq!(first_row(&rows(35), area, &mut view), 25);
        // 撤销藏起了几轮，只剩 27 行：已经顶上去的不掉下来。
        assert_eq!(first_row(&rows(27), area, &mut view), 25);
        // 接着发出一句话，多了 4 行：不往回跳，照旧只往下走。
        view.follow();
        assert_eq!(first_row(&rows(31), area, &mut view), 25);
        // 滚上去看，再发一句话：回到最底下。
        view.top = Some(5);
        assert_eq!(first_row(&rows(31), area, &mut view), 5);
        view.follow();
        assert_eq!(first_row(&rows(36), area, &mut view), 26);
    }

    #[test]
    fn a_menu_opening_and_closing_does_not_leave_a_gap() {
        let tall = Rect::new(0, 0, 10, 10);
        let short = Rect::new(0, 0, 10, 5);
        let mut view = BodyView::default();
        assert_eq!(first_row(&rows(30), tall, &mut view), 20);
        // 命令列表打开，视口矮了 5 行：贴着底边，往上让。
        assert_eq!(first_row(&rows(30), short, &mut view), 25);
        // 列表关掉，视口又高了：内容跟着下来，不留空白。
        assert_eq!(first_row(&rows(30), tall, &mut view), 20);
    }

    #[test]
    fn a_wider_window_rewraps_and_does_not_hold_the_old_row_numbers() {
        let narrow = Rect::new(0, 0, 40, 30);
        let wide = Rect::new(0, 0, 120, 60);
        let mut view = BodyView::default();
        assert_eq!(first_row(&rows(200), narrow, &mut view), 170);
        // 最大化：宽了，200 行折成 100 行，视口高了一倍。照最新的露满，不只剩一行。
        assert_eq!(first_row(&rows(100), wide, &mut view), 40);
    }

    #[test]
    fn short_content_stays_at_the_top_when_the_view_gets_shorter() {
        let tall = Rect::new(0, 0, 10, 20);
        let short = Rect::new(0, 0, 10, 17);
        let mut view = BodyView::default();
        assert_eq!(first_row(&rows(3), tall, &mut view), 0);
        // 开始回答，运行状态行那一块长出来，视口矮了 3 行：放得下的照旧从第一行露起。
        assert_eq!(first_row(&rows(3), short, &mut view), 0);
    }

    #[test]
    fn the_end_of_a_turn_lets_the_rows_settle_once() {
        let area = Rect::new(0, 0, 10, 10);
        let mut view = BodyView::default();
        assert_eq!(first_row(&rows(30), area, &mut view), 20);
        // 思考、工具收成一行，少了 8 行：回答进行中不动。
        assert_eq!(first_row(&rows(22), area, &mut view), 20);
        // 一轮结束：放开一次，上面的行补满空白。
        view.settle();
        assert_eq!(first_row(&rows(22), area, &mut view), 12);
    }

    #[test]
    fn ctrl_l_empties_the_view_and_keeps_the_rows() {
        let area = Rect::new(0, 0, 10, 10);
        let mut view = BodyView::default();
        view.rows = rows(30);
        view.clear();
        // 30 行全顶上去，视口是空的；往回滚还在。
        assert_eq!(first_row(&rows(30), area, &mut view), 30);
        // 新的字从空着的视口顶上往下长。
        assert_eq!(first_row(&rows(33), area, &mut view), 30);
        view.top = Some(0);
        assert_eq!(first_row(&rows(33), area, &mut view), 0);
    }

    #[test]
    fn after_ctrl_l_a_shorter_view_does_not_push_new_rows_up() {
        let tall = Rect::new(0, 0, 10, 10);
        let short = Rect::new(0, 0, 10, 7);
        let mut view = BodyView::default();
        view.rows = rows(30);
        view.clear();
        assert_eq!(first_row(&rows(30), tall, &mut view), 30);
        // 发出一句话（4 行），运行状态行那一块长出来，视口矮了 3 行：还从清屏那一行露起，不切掉开头。
        assert_eq!(first_row(&rows(34), short, &mut view), 30);
        assert_eq!(first_row(&rows(35), short, &mut view), 30);
        // 一轮结束，那一块收起：照旧从清屏那一行露起。
        assert_eq!(first_row(&rows(35), tall, &mut view), 30);
        // 新的字多到放不下了，才贴着底边走。
        assert_eq!(first_row(&rows(45), short, &mut view), 38);
    }

    #[test]
    fn page_keys_move_half_a_screen() {
        let area = Rect::new(0, 0, 10, 10);
        let mut view = BodyView::default();
        view.area = area;
        // 画的时候会把露出的第一行记进 `view.first`，这里照做。
        let draw = |view: &mut BodyView| {
            view.first = first_row(&rows(50), area, view);
            view.first
        };
        assert_eq!(draw(&mut view), 40);
        view.page(false);
        assert_eq!(draw(&mut view), 35, "翻半屏");
        view.page(true);
        assert_eq!(draw(&mut view), 40);
        assert!(view.top.is_none(), "翻到底又跟着最新的");
    }

    #[test]
    fn scrolling_after_ctrl_l_does_not_bring_the_rows_back() {
        let area = Rect::new(0, 0, 10, 10);
        let mut view = BodyView::default();
        view.rows = rows(30);
        view.clear();
        assert_eq!(first_row(&rows(30), area, &mut view), 30);
        // 往下滚：还是空的。
        view.top = Some(33);
        assert_eq!(first_row(&rows(30), area, &mut view), 30);
        // 往上滚三行：看得到之前的最后三行。
        view.top = Some(27);
        assert_eq!(first_row(&rows(30), area, &mut view), 27);
        // 滚回底：又是空的。
        view.top = Some(30);
        assert_eq!(first_row(&rows(30), area, &mut view), 30);
    }
}
