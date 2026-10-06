//! 画一帧：上面是正文，下面是圆角的输入框，框下面一行是用量。
//!
//! 版式照网页的聊天输入框：框居中、跟着窗口变宽，正文和框一样宽；框里只有文字，
//! 框下面一行左边是模式和模型，右边是临时的状态和用量。

mod agents;
mod background;
mod body;
mod bottom_button;
mod compaction_rows;
mod diff_rows;
mod done_row;
mod drawer;
pub mod effort_list;
mod figure_rows;
mod footer;
mod foreign_rows;
pub mod help;
mod history;
mod home;
mod input_box;
mod job_rows;
pub mod languages;
mod link_card;
mod margins;
mod mascot_view;
mod md_cache;
pub mod model_list;
mod panel;
pub mod session_list;
pub mod settings;
mod sidebar;

pub use history::{index_at as history_index_at, lines as history_lines};
pub use menu::{index_at as menu_index_at, rows as menu_rows};
mod mention;
mod menu;
pub mod row_cache;
pub mod rows;
mod status;
mod timeline;
mod undo_rows;
pub use timeline::release_on_fold;
mod user_rows;
pub mod wide;

#[cfg(test)]
mod rulers;
#[cfg(test)]
mod test_support;

use ratatui::Frame;
use ratatui::layout::Rect;

use crate::app::App;
use crate::config::Layout;

/// 一帧里各块的位置。
#[derive(Debug, Clone, Copy, Default)]
pub struct Areas {
    /// 正文。左右和输入框对齐。
    pub body: Rect,
    /// 输入框上面的三样（斜杠命令列表、历史列表、后台面板）连框：贴在输入框上面，没开时高是 0。左右和输入框的边对齐。
    pub menu: Rect,
    /// 那三样框里放字的那一块：左右和输入框里的字对齐（`panel::inside`）。
    pub menu_text: Rect,
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
    /// 回到底部的按钮；在底部、放不下时是空的（`bottom_button.rs`）。
    pub bottom: Rect,
    /// 侧边栏里的短编号那一行：点它复制完整的会话编号（第 7 条）。
    pub session_id: Rect,
}

/// 算出各块的位置。输入框的高度跟着文字的行数走，所以要先定宽度再定高度：`rows` 照框里的字多宽交回占几行
/// （抽屉开着时是抽屉的行数）。`menu_rows` 是列表要露出几行。
/// `running` 在回答，`queued` 排着几条消息，`agent_rows` 是子代理状态行占几行（连上面的空行），
/// `todo_rows` 是窄屏常驻的待办占几行（不连下面的空行）。
#[allow(clippy::too_many_arguments)]
pub fn areas(
    area: Rect,
    rows: &dyn Fn(u16) -> u16,
    layout: &Layout,
    menu_rows: u16,
    running: bool,
    queued: u16,
    agent_rows: u16,
    todo_rows: u16,
) -> Areas {
    let width = box_width(area.width, layout);
    let x = area.x + (area.width - width) / 2;
    let m = margins::margins(layout, area.width);
    let pad_left = m.pad_left;
    let text_width = width.saturating_sub(2 + pad_left + m.pad_right).max(1);
    let rows = rows(text_width);
    // 上下两条边和文字。
    let height = rows + 2;
    let footer_y = area.bottom().saturating_sub(1 + agent_rows);
    let frame = Rect::new(x, footer_y.saturating_sub(height), width, height).intersection(area);
    let inner_x = frame.x + 1 + pad_left;
    let text = Rect::new(inner_x, frame.y + 1, text_width, rows).intersection(frame);
    let footer = Rect::new(inner_x, footer_y, text_width, 1).intersection(area);
    // 列表是覆盖层：贴着输入框、盖在正文底部上，正文和下面那几样照没开框时的位置（`tui.md`「斜杠命令列表」第 1 条，
    // 2026-10-02 项目主人定：原来把正文往上推，高度一变正文就上下跳）。只用输入框上面剩下的行（「窗口小的时候」
    // 第 1 条）。
    let menu = above(frame.y, area.y, menu_rows);
    let menu = Rect::new(frame.x, menu.0, frame.width, menu.1);
    // 正文和下面那一块之间：在回答时是 空一行 · 运行状态行 · 排队的消息 · 空一行，再接输入框；没在回答时只空一行
    // （`tui.md`「运行状态行和排队的消息」）。
    // 窄屏的待办常驻在列表（没开时是输入框）上面，下面空一行（`tui.md`「后台命令、子代理和侧边栏」第 4 条）。
    let (todo_y, todo_rows) = above(frame.y.saturating_sub(1), area.y, todo_rows);
    let todo_y = if todo_rows > 0 { todo_y } else { frame.y };
    let block = if running { 3 + queued } else { 1 };
    let block_y = todo_y.saturating_sub(block).max(area.y);
    // 运行状态行那一块放不下的不画，不盖到下面去。
    let free = Rect::new(inner_x, area.y, text_width, todo_y.saturating_sub(area.y));
    let pulse_y = if running { block_y + 1 } else { block_y };
    let pulse = Rect::new(inner_x, pulse_y, text_width, 1).intersection(free);
    let queued_rows = if running { queued } else { 0 };
    let queued = Rect::new(inner_x, pulse_y + 1, text_width, queued_rows).intersection(free);
    // 正文和输入框一样宽，上面空出 top_gap 行。
    let top = (area.y + layout.top_gap).min(block_y);
    let body = Rect::new(frame.x, top, frame.width, block_y.saturating_sub(top));
    Areas {
        body,
        menu,
        menu_text: list_text(menu, inner_x, text_width, layout),
        frame,
        text,
        footer,
        pulse,
        queued,
        agents: agents_area(inner_x, text_width, footer_y + 1, agent_rows, layout)
            .intersection(area),
        todo: Rect::new(inner_x, todo_y, text_width, todo_rows).intersection(area),
        ..Areas::default()
    }
}

/// 输入框上面三样框里放字的那一块：选中的 `❯` 和输入框的提示符同一列，名字和框里打的字同一列
/// （`tui.md`「斜杠命令列表」第 3 条）。`text_x`、`text_width` 是输入框里放字的那一块。
pub(super) fn list_text(outer: Rect, text_x: u16, text_width: u16, layout: &Layout) -> Rect {
    let x = text_x
        .saturating_sub(prompt_width(layout))
        .max(outer.x.saturating_add(1));
    panel::inside(outer, x, text_width + (text_x - x))
}

/// 子代理状态行：每行前面那两格落在提示符那一列，`●` `○` 和框下面那一行的 `▣` 同一列（「后台命令、子代理和
/// 侧边栏」第 6 条）。`text_x`、`text_width` 是输入框里放字的那一块，`y`、`rows` 是从第几行起、几行。
pub(super) fn agents_area(
    text_x: u16,
    text_width: u16,
    y: u16,
    rows: u16,
    layout: &Layout,
) -> Rect {
    let lead = prompt_width(layout).min(text_x);
    Rect::new(text_x - lead, y, text_width + lead, rows)
}

/// 输入框的提示符（`layout.json` 的 `prompt`）占几列。
fn prompt_width(layout: &Layout) -> u16 {
    u16::try_from(unicode_width::UnicodeWidthStr::width(
        layout.prompt.as_str(),
    ))
    .unwrap_or(0)
}

/// 贴着 `bottom` 往上要 `rows` 行，顶多到 `top`：交回从第几行起、给了几行。
pub(super) fn above(bottom: u16, top: u16, rows: u16) -> (u16, u16) {
    let rows = rows.min(bottom.saturating_sub(top));
    (bottom - rows, rows)
}

/// 输入框（连边框）多宽：终端宽的几成，两边留白；太窄时、紧凑版面铺满。
pub(super) fn box_width(area_width: u16, layout: &Layout) -> u16 {
    let m = margins::margins(layout, area_width);
    if m.compact {
        return area_width;
    }
    let room = area_width.saturating_sub(m.side_gap * 2);
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
    let m = margins::margins(layout, area_width);
    box_width(area_width, layout)
        .saturating_sub(2 + m.pad_left + m.pad_right)
        .max(1)
}

/// 画一帧。顺手把各块的位置记进 `app`，鼠标事件要用。
pub fn draw(frame: &mut Frame, app: &mut App) {
    app.caret.begin();
    // 配置页开着：整屏归它，对话不画（图片也不画），关了照原样画回来（蓝图「配置页」第 1 条）。
    if let Some(page) = app.settings.as_mut() {
        settings::draw(frame, page, &app.config, &mut app.caret);
        return;
    }
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
    let drawer_open = app.drawers.open();
    // `@` 文件列表也占这个地方，开着时不开斜杠命令列表（「`@` 文件列表」第 1 条）。
    let mentions = crate::frame_log::section("mention", || app.mention_found());
    let matches = if app.history.open || app.panel.is_some() || drawer_open || mentions.is_some() {
        None
    } else {
        app.menu_matches()
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
    let agent_rows = agents::height(app.listed_agents().len(), app.config.layout.agent_rows);
    // 没有侧边栏（窄屏、首页）时，待办常驻在输入框上面（第 4 条）；有侧边栏时在侧边栏里。
    let todo_lines = if sidebar.width == 0 {
        // 首页的框收窄了：照首页框里的字宽排，不然长的一项被裁掉、没有「…」。
        let width = if home {
            home::text_width(main.width, &app.config.layout)
        } else {
            text_width(main.width, &app.config.layout)
        };
        sidebar::todo_lines(
            &app.board,
            &app.config,
            width,
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
    // 框里占几行：抽屉开着时是抽屉的行数，连边框最高半屏；带文字画的放得下整张为止，屏幕顶上留几行
    // （`tui.md`「确认和提问的抽屉」第 2 条）。
    let texts = &app.config.text.drawer;
    let open_drawer = app.drawers.current.as_ref();
    let half = (main.height / 2).saturating_sub(2).max(3);
    let drawer_max = match open_drawer {
        Some(d) if d.has_preview() => main
            .height
            .saturating_sub(app.config.layout.drawer_keep_rows + 4)
            .max(half),
        _ => half,
    };
    let box_rows = |w: u16| match open_drawer {
        Some(d) => drawer::rows(d, texts, w, drawer_max),
        None => app.input.rows(w),
    };
    // 空会话是首页：整组上下居中，吉祥物在中间（`tui.md`「空会话的首页」）。列表占几行交给 `place` 定位。
    let place = |menu_rows: u16| {
        if home {
            let look = &app.config.mascot;
            // 首页的吉祥物能关（「后台命令、子代理和侧边栏」第 7 条）。
            let mascot = if app.config.layout.mascot_home {
                (look.cols, look.rows)
            } else {
                (0, 0)
            };
            home::areas(
                main,
                &box_rows,
                &app.config.layout,
                mascot,
                menu_rows,
                agent_rows,
                todo_rows,
            )
        } else {
            areas(
                main,
                &box_rows,
                &app.config.layout,
                menu_rows,
                running,
                count,
                agent_rows,
                todo_rows,
            )
        }
    };
    // 输入框上面的列表最多能占几行（连框）：放不下的由各列表自己瘦身（「窗口小的时候」第 1 条）。框里的几条照这一帧
    // 量出来的宽度排：首页的输入框窄一些（「斜杠命令列表」第 3 条）。
    let probe = place(u16::MAX);
    let room = probe.menu.height;
    let inner = panel::room(room);
    let now = std::time::Instant::now();
    let width = probe.menu_text.width;
    // 历史列表排成的行：占几行、画什么都照它（`history.rs` 的 `lines`）。
    let (history_chrome, history_rows) = if app.history.open {
        let found = app.history.matches(app.input.sent());
        history::lines(
            &app.history,
            &found,
            width,
            &app.config,
            std::time::SystemTime::now(),
            usize::from(inner),
        )
    } else {
        Default::default()
    };
    // 后台面板排成的行（第 3 条）；帮助框（`/help`）一样放在这个位置，点它哪一行都不算点中。
    let (panel_chrome, panel_lines, panel_rows) = match app.panel {
        Some(crate::app::Panel::Help { scroll }) => {
            let (chrome, lines) = help::lines(&app.config, width, scroll, usize::from(inner));
            let map = vec![None; lines.len()];
            (chrome, lines, map)
        }
        Some(crate::app::Panel::Models) => match &app.model_list {
            Some(list) => model_list::lines(
                list,
                app.transcript.model_ref(),
                &app.config,
                width,
                usize::from(inner),
            ),
            None => Default::default(),
        },
        Some(crate::app::Panel::Sessions) => match &app.session_list {
            Some(list) => session_list::lines(
                list,
                app.main_session().as_deref(),
                &app.cwd,
                &app.config,
                width,
                usize::from(inner),
                spin_frame(app),
            ),
            None => Default::default(),
        },
        Some(crate::app::Panel::Effort { selected }) => effort_list::lines(
            app.efforts.as_ref(),
            selected,
            &app.config,
            width,
            usize::from(inner),
        ),
        Some(crate::app::Panel::Language { selected }) => languages::lines(
            &app.config,
            &app.system_language,
            selected,
            width,
            usize::from(inner),
        ),
        _ => background::lines(
            app.panel,
            &app.board,
            &app.config,
            width,
            now,
            usize::from(inner),
        ),
    };
    let menu_shown = menu::rows(app.config.layout.menu_rows, inner);
    // 框里的几条，放得下框的加上上下两条边（`tui.md`「斜杠命令列表」第 3 条）。
    let items = if app.panel.is_some() {
        panel_lines.len()
    } else if app.history.open {
        history_rows.len()
    } else if let Some(found) = &mentions {
        found.items.len().clamp(1, menu_shown)
    } else {
        matches.as_ref().map_or(0, |m| m.len().min(menu_shown))
    };
    let menu_rows = panel::height(u16::try_from(items).unwrap_or(u16::MAX), room);
    let mut areas = place(menu_rows);
    areas.sidebar = sidebar;
    app.areas = areas;
    app.panel_rows = panel_rows;
    use crate::frame_log::section;
    if home {
        // 先画输入框（吉祥物照输入光标转头），再画吉祥物；开着列表时吉祥物已经让到列表上面。
        section("input", || input_box::draw_box(frame, areas, app, home));
        section("home", || home::draw(frame, areas, app));
    } else {
        section("body", || body::draw(frame, areas, app));
        section("input", || input_box::draw_box(frame, areas, app, home));
    }
    // 待办、运行状态行、排队的消息先画：输入框上面的框是覆盖层，开着时盖在它们上面（「斜杠命令列表」第 1 条）。
    frame.render_widget(ratatui::widgets::Paragraph::new(todo_lines), areas.todo);
    section("status", || status::draw(frame, areas.pulse, app));
    status::queued(frame, areas.queued, &queued, &app.config.layout.queued_mark);
    if let Some(matches) = &matches {
        let rows = menu::rows(app.config.layout.menu_rows, areas.menu_text.height);
        let (chrome, lines) = menu::lines(
            matches,
            app.menu.selected,
            app.menu.pinned,
            rows,
            areas.menu_text.width,
            &app.config.text.menu,
        );
        menu::draw(frame, areas.menu, areas.menu_text, chrome, lines);
    }
    if let Some(found) = &mentions {
        let rows = menu::rows(app.config.layout.menu_rows, areas.menu_text.height);
        let (chrome, lines) = mention::lines(
            found,
            app.mention.selected,
            app.mention.pinned,
            rows,
            areas.menu_text.width,
            &app.config.text.mention,
        );
        menu::draw(frame, areas.menu, areas.menu_text, chrome, lines);
    }
    if app.history.open {
        history::draw(
            frame,
            areas.menu,
            areas.menu_text,
            history_chrome,
            history_rows,
        );
    }
    background::draw(
        frame,
        areas.menu,
        areas.menu_text,
        panel_chrome,
        panel_lines,
    );
    app.areas.button = section("footer", || footer::draw(frame, areas.footer, app));
    // 不在首页才有正文可翻。
    app.areas.bottom = if home {
        Rect::default()
    } else {
        // 只用正文那一边（有侧边栏时不碰它的竖线）。
        bottom_button::draw(frame, app, main)
    };
    section("agents", || agents::draw(frame, areas.agents, app));
    section("sidebar", || sidebar::draw(frame, sidebar, app));
    // 提示最后画，浮在正文上面；底边紧贴输入框，在回答时紧贴运行状态行（`tui.md`「提示」）；开着框时贴着框的上边。
    let toast_bottom = if running {
        areas.pulse.y.saturating_sub(1)
    } else {
        areas.body.bottom()
    };
    let toast_bottom = if areas.menu.height > 0 {
        toast_bottom.min(areas.menu.y)
    } else {
        toast_bottom
    };
    status::toast(frame, areas.text.x, toast_bottom, areas.text.width, app);
}

#[cfg(test)]
mod tests;

/// 转圈转到第几帧：照界面起来以后过了多久，一帧 `timeline.spinner_ms`。
fn spin_frame(app: &App) -> usize {
    let spinner_ms = app.config.timeline.spinner_ms.max(1);
    usize::try_from(app.started.elapsed().as_millis() / u128::from(spinner_ms)).unwrap_or(0)
}
