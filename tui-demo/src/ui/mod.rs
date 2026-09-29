//! 画一帧：上面是正文，下面是圆角的输入框，框下面一行是用量。
//!
//! 版式照网页的聊天输入框：框居中、跟着窗口变宽，正文和框一样宽；框里只有文字，
//! 框下面一行左边是模式和模型，右边是临时的状态和用量。

mod agents;
mod background;
mod body;
mod diff_rows;
mod figure_rows;
mod footer;
mod history;
mod home;
mod job_rows;
mod mascot_view;
mod sidebar;

pub use history::{index_at as history_index_at, lines as history_lines};
mod menu;
pub mod rows;
mod status;
mod timeline;

#[cfg(test)]
mod test_support;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::{Block, BorderType, Paragraph};

use crate::app::App;
use crate::config::{Config, Layout};
use crate::core::Level;
use crate::input::{InputBox, VisualLine};
use crate::theme;

use unicode_width::UnicodeWidthStr;

/// 一帧里各块的位置。
#[derive(Debug, Clone, Copy, Default)]
pub struct Areas {
    /// 正文。左右和输入框对齐。
    pub body: Rect,
    /// 斜杠命令列表：贴在输入框上面，没开时高是 0。左右和框里的字对齐。
    pub menu: Rect,
    /// 输入框，含边框。
    pub frame: Rect,
    /// 输入框里放文字的地方。
    pub text: Rect,
    /// 输入框下面一行：权限级别、模型、用量。左右和框里的字对齐。
    pub footer: Rect,
    /// 运行状态行：在回答时写「深度求索」和用时，提示靠右；没在回答时只有提示，靠左（`tui.md`「运行状态行和排队的消息」）。
    pub pulse: Rect,
    /// 排队的消息，一条一行，列在运行状态行下面；没有的高是 0。
    pub queued: Rect,
    /// 首页的吉祥物；不在首页、放不下时高是 0（`tui.md`「空会话的首页」）。
    pub mascot: Rect,
    /// 框下面那一行中间的后台按钮；没有时宽是 0（`tui.md`「后台命令、子代理和侧边栏」第 1 条）。
    pub button: Rect,
    /// 窄屏时输入框上面常驻的待办；没有时高是 0（第 4 条）。
    pub todo: Rect,
    /// 首页框下面那一行再下面（空一行）的工作目录；不在首页是空的（「空会话的首页」第 2 条）。
    pub cwd: Rect,
    /// 子代理状态行（连上面的空行）；没有时高是 0（第 5 条）。
    pub agents: Rect,
    /// 右边的侧边栏（不含竖线）；窗口窄时宽是 0（第 7 条）。
    pub sidebar: Rect,
    /// 侧边栏里的短编号那一行：点它复制完整的会话编号（第 7 条）。
    pub session_id: Rect,
}

/// 算出各块的位置。输入框的高度跟着文字的行数走，所以要先定宽度再定高度。`menu_rows` 是列表要露出几行。
/// `running` 在回答，`queued` 排着几条消息，`agent_rows` 是子代理状态行占几行（连上面的空行），
/// `todo_rows` 是窄屏常驻的待办占几行（不连下面的空行）。
#[allow(clippy::too_many_arguments)]
pub fn areas(
    area: Rect,
    input: &InputBox,
    layout: &Layout,
    menu_rows: u16,
    running: bool,
    queued: u16,
    agent_rows: u16,
    todo_rows: u16,
) -> Areas {
    let width = box_width(area.width, layout);
    let x = area.x + (area.width - width) / 2;
    let pad_left = layout.pad_left;
    let text_width = width.saturating_sub(2 + pad_left + layout.pad_right).max(1);
    let rows = input.rows(text_width);
    // 上下两条边和文字。
    let height = rows + 2;
    let footer_y = area.bottom().saturating_sub(1 + agent_rows);
    let frame = Rect::new(x, footer_y.saturating_sub(height), width, height).intersection(area);
    let inner_x = frame.x + 1 + pad_left;
    let text = Rect::new(inner_x, frame.y + 1, text_width, rows).intersection(frame);
    let footer = Rect::new(inner_x, footer_y, text_width, 1).intersection(area);
    // 列表出现时把正文往上推，不盖住正文（13-终端界面.md H4）。
    let menu_y = frame.y.saturating_sub(menu_rows);
    let menu = Rect::new(inner_x, menu_y, text_width, menu_rows).intersection(area);
    // 正文和下面那一块之间：在回答时是 空一行 · 运行状态行 · 排队的消息 · 空一行，再接输入框；没在回答时只空一行
    // （`tui.md`「运行状态行和排队的消息」）。
    // 窄屏的待办常驻在列表（没开时是输入框）上面，下面空一行（`tui.md`「后台命令、子代理和侧边栏」第 4 条）。
    let todo_gap = if todo_rows > 0 { todo_rows + 1 } else { 0 };
    let todo_y = menu_y.saturating_sub(todo_gap);
    let block = if running { 3 + queued } else { 1 };
    let block_y = todo_y.saturating_sub(block);
    let pulse_y = if running { block_y + 1 } else { block_y };
    let pulse = Rect::new(inner_x, pulse_y, text_width, 1).intersection(area);
    let queued_rows = if running { queued } else { 0 };
    let queued = Rect::new(inner_x, pulse_y + 1, text_width, queued_rows).intersection(area);
    // 正文和输入框一样宽，上面空出 top_gap 行。
    let top = (area.y + layout.top_gap).min(block_y);
    let body = Rect::new(frame.x, top, frame.width, block_y.saturating_sub(top));
    Areas {
        body,
        menu,
        frame,
        text,
        footer,
        pulse,
        queued,
        agents: Rect::new(inner_x, footer_y + 1, text_width, agent_rows).intersection(area),
        todo: Rect::new(inner_x, todo_y, text_width, todo_rows).intersection(area),
        ..Areas::default()
    }
}

/// 输入框（连边框）多宽：终端宽的几成，两边留白；太窄时铺满。
pub(super) fn box_width(area_width: u16, layout: &Layout) -> u16 {
    let room = area_width.saturating_sub(layout.side_gap * 2);
    let wanted = u16::try_from(u32::from(area_width) * u32::from(layout.width_percent) / 100)
        .unwrap_or(area_width);
    let width = wanted.min(room);
    if width < layout.narrow_below {
        area_width
    } else {
        width
    }
}

/// 框里的字（也是命令列表、历史列表、正文内容）多宽。
fn text_width(area_width: u16, layout: &Layout) -> u16 {
    box_width(area_width, layout)
        .saturating_sub(2 + layout.pad_left + layout.pad_right)
        .max(1)
}

/// 画一帧。顺手把各块的位置记进 `app`，鼠标事件要用。
pub fn draw(frame: &mut Frame, app: &mut App) {
    // 不在首页、够宽时右边分出侧边栏，别的都画在主列里（`tui.md`「后台命令、子代理和侧边栏」第 7 条）。
    let home = app.home();
    let (main, sidebar) = if home {
        let area = frame.area();
        (area, Rect::new(area.right(), area.y, 0, area.height))
    } else {
        sidebar::split(frame.area(), &app.config.layout)
    };
    // 输入历史列表开着时不看斜杠命令：两个不同时开，占同一个地方（`tui.md`「输入历史列表」）。
    // 后台面板也占那个地方，开着时两个列表都不开。
    let matches = if app.history.open || app.panel.is_some() {
        None
    } else {
        app.menu_matches()
    };
    // 历史列表排成的行：占几行、画什么都照它（`history.rs` 的 `lines`）。
    let history_rows = if app.history.open {
        let found = app.history.matches(app.input.sent());
        let width = text_width(main.width, &app.config.layout);
        history::lines(&app.history, &found, width, &app.config)
    } else {
        Vec::new()
    };
    // 后台面板、待办面板排成的行（第 3、4 条）。
    let (panel_lines, panel_rows) = background::lines(
        app.panel,
        &app.board,
        &app.config,
        text_width(main.width, &app.config.layout),
        std::time::Instant::now(),
    );
    app.panel_rows = panel_rows;
    let menu_rows = if app.panel.is_some() {
        u16::try_from(panel_lines.len()).unwrap_or(u16::MAX)
    } else if app.history.open {
        u16::try_from(history_rows.len()).unwrap_or(u16::MAX)
    } else {
        let shown = matches
            .as_ref()
            .map_or(0, |m| m.len().min(app.config.layout.menu_rows));
        u16::try_from(shown).unwrap_or(0)
    };
    let running = app.transcript.running.is_some();
    let queued: Vec<String> = app
        .transcript
        .entries
        .iter()
        .filter(|e| e.queued && !e.hidden)
        .map(|e| e.text.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect();
    let count = u16::try_from(queued.len()).unwrap_or(u16::MAX);
    let agent_rows = agents::height(&app.board, app.config.layout.agent_rows);
    // 没有侧边栏（窄屏、首页）时，待办常驻在输入框上面（第 4 条）；有侧边栏时在侧边栏里。
    let todo_lines = if sidebar.width == 0 {
        sidebar::todo_lines(
            &app.board,
            &app.config,
            text_width(main.width, &app.config.layout),
            app.config.layout.todo_rows,
            app.todo_full,
        )
    } else {
        Vec::new()
    };
    let todo_rows = u16::try_from(todo_lines.len()).unwrap_or(u16::MAX);
    // 输入框从有字变空时换一条提示。
    app.tips
        .see(app.input.editor.is_empty(), app.config.text.tips.len());
    // 空会话是首页：整组上下居中，吉祥物在中间（`tui.md`「空会话的首页」）。
    let mut areas = if home {
        let look = &app.config.mascot;
        // 首页的吉祥物能关（「后台命令、子代理和侧边栏」第 7 条）。
        let mascot = if app.config.layout.mascot_home {
            (look.cols, look.rows)
        } else {
            (0, 0)
        };
        home::areas(
            main,
            &app.input,
            &app.config.layout,
            mascot,
            menu_rows,
            agent_rows,
            todo_rows,
        )
    } else {
        areas(
            main,
            &app.input,
            &app.config.layout,
            menu_rows,
            running,
            count,
            agent_rows,
            todo_rows,
        )
    };
    areas.sidebar = sidebar;
    app.areas = areas;
    if home {
        // 先画输入框（吉祥物照输入光标转头），再画吉祥物；开着列表时吉祥物已经让到列表上面。
        draw_input(
            frame,
            areas,
            &mut app.input,
            &app.config,
            app.transcript.level,
            placeholder(&app.config, home, app.tips.at()),
        );
        home::draw(frame, areas, app);
    } else {
        body::draw(frame, areas, app);
        draw_input(
            frame,
            areas,
            &mut app.input,
            &app.config,
            app.transcript.level,
            placeholder(&app.config, home, app.tips.at()),
        );
    }
    if let Some(matches) = &matches {
        menu::draw(frame, areas.menu, matches, app.menu.selected);
    }
    if app.history.open {
        history::draw(frame, areas.menu, history_rows);
    }
    background::draw(frame, areas.menu, panel_lines);
    app.areas.button = footer::draw(frame, areas.footer, app);
    frame.render_widget(ratatui::widgets::Paragraph::new(todo_lines), areas.todo);
    agents::draw(frame, areas.agents, app);
    sidebar::draw(frame, sidebar, app);
    status::draw(frame, areas.pulse, app);
    status::queued(frame, areas.queued, &queued, &app.config.layout.queued_mark);
    // 提示最后画，浮在正文上面；底边紧贴输入框，在回答时紧贴运行状态行（`tui.md`「提示」）。
    let toast_bottom = if running {
        areas.pulse.y.saturating_sub(1)
    } else {
        areas.body.bottom()
    };
    status::toast(frame, areas.text.x, toast_bottom, areas.text.width, app);
}

fn draw_input(
    frame: &mut Frame,
    areas: Areas,
    input: &mut InputBox,
    config: &Config,
    level: Level,
    placeholder: &str,
) {
    frame.render_widget(
        Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(theme::dim()),
        areas.frame,
    );
    let scroll = input.place(areas.text);
    let editor = &input.editor;
    let body = if editor.is_empty() {
        vec![Line::styled(placeholder, theme::dim())]
    } else {
        let lines = input.lines(areas.text.width);
        lines
            .iter()
            .skip(scroll)
            .take(usize::from(areas.text.height))
            .map(|l| styled_line(editor.text(), *l, editor.selection()))
            .collect()
    };
    frame.render_widget(Paragraph::new(body), areas.text);
    // 暂存着东西：第一行最右边暗色写一个标记；和第一行的字挤不下就不写。
    if input.stashed() {
        let mark = config.text.stashed.as_str();
        let first = if editor.is_empty() {
            placeholder.width()
        } else {
            input
                .lines(areas.text.width)
                .get(scroll)
                .map_or(0, |l| editor.text()[l.start..l.end].width())
        };
        // 标记和字之间至少空一列。
        if first + mark.width() < usize::from(areas.text.width) {
            let row = Rect::new(areas.text.x, areas.text.y, areas.text.width, 1);
            frame.render_widget(
                Paragraph::new(Line::styled(mark, theme::dim()).right_aligned()),
                row,
            );
        }
    }
    // 提示符画在第一行文字的左边，住在 pad_left 那几列里，颜色跟着权限级别；放不下就不画。
    // 暂存着东西换成软盘（`tui.md`「输入框」第 6 条）。
    let prompt = if input.stashed() {
        config.layout.stash_prompt.as_str()
    } else {
        config.layout.prompt.as_str()
    };
    let prompt_style = theme::level(level);
    let width = u16::try_from(prompt.width()).unwrap_or(u16::MAX);
    if scroll == 0 && config.layout.pad_left > width {
        let at = Rect::new(areas.text.x - width, areas.text.y, width, 1);
        frame.render_widget(Paragraph::new(Line::styled(prompt, prompt_style)), at);
    }
    if let Some(pos) = input.cursor_position() {
        frame.set_cursor_position(pos);
    }
}

/// 输入框空着时写的提示（`tui.md`「输入框」第 9 条）：首页固定写怎么切权限级别，别处照轮到的那一条。
fn placeholder(config: &Config, home: bool, tip: usize) -> &str {
    if home {
        return &config.text.home_placeholder;
    }
    config.text.tips.get(tip).map_or("", String::as_str)
}

/// 一行字，选中的那一段反色。
fn styled_line(text: &str, line: VisualLine, selection: Option<(usize, usize)>) -> Line<'_> {
    use ratatui::text::Span;
    let Some((s, e)) = selection else {
        return Line::raw(&text[line.start..line.end]);
    };
    let s = s.clamp(line.start, line.end);
    let e = e.clamp(line.start, line.end);
    Line::from(vec![
        Span::raw(&text[line.start..s]),
        Span::styled(&text[s..e], theme::selected()),
        Span::raw(&text[e..line.end]),
    ])
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use ratatui::layout::Rect;

    use super::areas;
    use crate::config::Config;
    use crate::input::InputBox;

    #[test]
    fn while_answering_one_blank_above_the_status_and_one_below_it() {
        let layout = Config::builtin().unwrap().layout;
        let input = InputBox::new(8, Duration::from_millis(400));
        let a = areas(Rect::new(0, 0, 100, 40), &input, &layout, 0, true, 2, 0, 0);
        // 正文 · 空一行 · 运行状态行 · 两条排队的 · 空一行 · 输入框（`tui.md`「运行状态行和排队的消息」）。
        assert_eq!(a.pulse.y, a.body.bottom() + 1, "上面空一行");
        assert_eq!((a.queued.y, a.queued.height), (a.pulse.y + 1, 2));
        assert_eq!(a.frame.y, a.queued.bottom() + 1, "下面空一行");
        // 没有排队的：空行直接在运行状态行下面。
        let b = areas(Rect::new(0, 0, 100, 40), &input, &layout, 0, true, 0, 0, 0);
        assert_eq!(b.frame.y, b.pulse.y + 2);
        // 没在回答：只空一行。
        let c = areas(Rect::new(0, 0, 100, 40), &input, &layout, 0, false, 0, 0, 0);
        assert_eq!(c.frame.y, c.body.bottom() + 1);
    }

    #[test]
    fn the_narrow_todo_list_sits_above_the_box_with_a_blank_between() {
        let layout = Config::builtin().unwrap().layout;
        let input = InputBox::new(8, Duration::from_millis(400));
        // 没在回答：正文 · 空一行 · 待办 6 行 · 空一行 · 输入框。
        let a = areas(Rect::new(0, 0, 100, 40), &input, &layout, 0, false, 0, 0, 6);
        assert_eq!((a.todo.height, a.todo.bottom() + 1), (6, a.frame.y));
        assert_eq!(a.todo.y, a.body.bottom() + 1);
        assert_eq!(a.todo.x, a.text.x, "和框里的字左对齐");
        // 开着列表：列表在待办和输入框之间。
        let b = areas(Rect::new(0, 0, 100, 40), &input, &layout, 4, false, 0, 0, 6);
        assert_eq!(b.menu.bottom(), b.frame.y);
        assert_eq!(b.todo.bottom() + 1, b.menu.y);
        // 在回答：运行状态行那一块在待办上面。
        let c = areas(Rect::new(0, 0, 100, 40), &input, &layout, 0, true, 0, 0, 6);
        assert_eq!(c.pulse.y + 2, c.todo.y);
    }

    #[test]
    fn the_home_screen_always_shows_the_level_tip() {
        let config = Config::builtin().unwrap();
        // 首页：固定这一条，不管轮到哪一条（`tui.md`「输入框」第 9 条）。
        assert_eq!(super::placeholder(&config, true, 3), "Tab 切换权限级别");
        // 离开首页：照轮换的。
        assert_eq!(super::placeholder(&config, false, 3), config.text.tips[3]);
    }
}
