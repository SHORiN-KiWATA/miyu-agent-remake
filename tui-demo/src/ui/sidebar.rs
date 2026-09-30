//! 侧边栏（蓝图 `tui.md`「后台命令、子代理和侧边栏」第 6 条）：窗口够宽时右边一栏，吉祥物在上，待办在下；
//! 和主列之间一根暗色竖线。

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::app::App;
use crate::config::{Config, Layout, TodoMarks};
use crate::core::Usage;
use crate::jobs::{Board, Todo, TodoState};
use crate::side_select::TextRow;
use crate::transcript::Transcript;
use crate::{meter, theme};

/// 分出主列和侧边栏（侧边栏不含竖线）；窗口窄时侧边栏宽是 0。
pub fn split(area: Rect, layout: &Layout) -> (Rect, Rect) {
    if area.width < layout.sidebar_from {
        return (area, Rect::new(area.right(), area.y, 0, area.height));
    }
    let side = layout.sidebar_width;
    let main = Rect::new(area.x, area.y, area.width - side - 1, area.height);
    let sidebar = Rect::new(area.right() - side, area.y, side, area.height);
    (main, sidebar)
}

/// 待办那一段：`待办 2/5`，每一项一个记号。没有待办、全做完了是空的。
/// `full` 时全部列出、长的折行；不然项目最多 `rows` 行：放得下的一项一行，放不下的只露正在做的那几项，
/// 做完的收成一行、后面放不下的收成一行，长的截掉加 `…`（`tui.md`「后台命令、子代理和侧边栏」第 4 条）。
pub fn todo_lines(
    board: &Board,
    config: &Config,
    width: u16,
    rows: usize,
    full: bool,
) -> Vec<Line<'static>> {
    let words = &config.text.jobs;
    let marks = &config.layout.todo_marks;
    let Some((done, total)) = board.todo_progress().filter(|(done, total)| done < total) else {
        return Vec::new();
    };
    let title = words
        .todo
        .replace("{done}", &done.to_string())
        .replace("{total}", &total.to_string());
    let mut out = vec![Line::styled(title, theme::dim())];
    if full {
        for todo in &board.todos {
            out.extend(item(todo, marks, width, true));
        }
        return out;
    }
    if board.todos.len() <= rows {
        for todo in &board.todos {
            out.extend(item(todo, marks, width, false));
        }
        return out;
    }
    // 放不下：做完的收成一行，接着没做完的，放不下的收成最后一行。
    let open: Vec<&Todo> = board
        .todos
        .iter()
        .filter(|t| t.state != TodoState::Done)
        .collect();
    let folded = usize::from(done > 0);
    if done > 0 {
        let text = words.todo_folded.replace("{count}", &done.to_string());
        out.push(Line::from(vec![
            Span::styled(marks.done.clone(), theme::dim()),
            Span::styled(text, theme::dim()),
        ]));
    }
    let room = rows.saturating_sub(folded);
    let shown = if open.len() <= room {
        open.len()
    } else {
        room.saturating_sub(1)
    };
    for todo in &open[..shown] {
        out.extend(item(todo, marks, width, false));
    }
    if shown < open.len() {
        let more = words
            .todo_more
            .replace("{count}", &(open.len() - shown).to_string());
        out.push(Line::styled(more, theme::dim()));
    }
    out
}

/// 一项：记号加字。`wrap` 时长的折行、和字对齐，不然截掉加 `…`。
fn item(todo: &Todo, marks: &TodoMarks, width: u16, wrap: bool) -> Vec<Line<'static>> {
    let (mark, style) = match todo.state {
        TodoState::Pending => (&marks.pending, theme::dim()),
        TodoState::Active => (&marks.active, theme::accent().add_modifier(Modifier::BOLD)),
        TodoState::Done => (
            &marks.done,
            theme::dim().add_modifier(Modifier::CROSSED_OUT),
        ),
    };
    let room = usize::from(width).saturating_sub(mark.width()).max(1);
    let lead_style = style.remove_modifier(Modifier::CROSSED_OUT);
    if !wrap {
        let text = super::rows::clip(&todo.text, u16::try_from(room).unwrap_or(1));
        return vec![Line::from(vec![
            Span::styled(mark.clone(), lead_style),
            Span::styled(text, style),
        ])];
    }
    let indent = " ".repeat(mark.width());
    wrap_text(&todo.text, room)
        .into_iter()
        .enumerate()
        .map(|(i, piece)| {
            let (lead, lead_style) = if i == 0 {
                (mark.clone(), lead_style)
            } else {
                (indent.clone(), Style::new())
            };
            Line::from(vec![
                Span::styled(lead, lead_style),
                Span::styled(piece, style),
            ])
        })
        .collect()
}

/// 按显示宽度折行。
fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut used = 0;
    for c in text.chars() {
        let w = c.width().unwrap_or(0);
        if used + w > width && used > 0 {
            out.push(String::new());
            used = 0;
        }
        if let Some(last) = out.last_mut() {
            last.push(c);
        }
        used += w;
    }
    out
}

/// 会话、工作目录、上下文、用量几段（`tui.md`「后台命令、子代理和侧边栏」第 7 条）：小标题加粗、内容缩进两格、
/// 一段之间空一行。会话名称当第一段的标题，下面是短编号；还是 0 的上下文、用量不写。
/// 一并交回短编号在第几行（点它复制完整编号）。
pub fn info_lines(
    t: &Transcript,
    total: &Usage,
    config: &Config,
    width: u16,
    cwd: &str,
) -> (Vec<Line<'static>>, Option<usize>) {
    let text = &config.text;
    let room = usize::from(width).saturating_sub(2).max(1);
    let bold = theme::side_title();
    let item = |s: String, style: Style| Line::styled(format!("  {s}"), style);
    // 标题一律蓝色，没起名字的也一样；内容一律暗（第 7 条）。
    let name = t.title.as_deref().unwrap_or(&text.untitled);
    let mut out = vec![Line::styled(super::rows::clip(name, width), bold)];
    let mut id_row = None;
    if let Some(session) = &t.session {
        let short: String = session.chars().take(8).collect();
        id_row = Some(out.len());
        out.push(item(text.side_id.replace("{id}", &short), theme::dim()));
    }
    out.push(Line::raw(""));
    out.push(Line::styled(text.side_cwd.clone(), bold));
    out.extend(
        wrap_path(cwd, room)
            .into_iter()
            .map(|l| item(l, theme::dim())),
    );
    if t.context > 0 {
        let used = meter::short(t.context);
        // 窗口照核心在订阅的回应里给的（`tui.md` 第 7 条），头不自己照模型名查。
        let window = t.limits.window;
        out.push(Line::raw(""));
        out.push(Line::styled(text.side_context.clone(), bold));
        match window {
            Some(window) => {
                let value = text
                    .side_context_value
                    .replace("{used}", &used)
                    .replace("{window}", &meter::short(window))
                    .replace("{percent}", &meter::percent_tenths(t.context, window));
                out.push(item(value, theme::dim()));
                let marks = &config.layout.bar;
                let line = t.limits.compaction_line;
                out.push(bar(t.context, window, line, marks.width.min(room), marks));
            }
            None => out.push(item(used, theme::dim())),
        }
        if t.cache.compactions > 0 {
            let compactions = text
                .side_compactions
                .replace("{n}", &t.cache.compactions.to_string());
            out.push(item(compactions, theme::dim()));
        }
    }
    // 用量连同子代理用的（`App::usage_total`，2026-09-30 项目主人）。
    let input = total.input();
    if input + total.output > 0 {
        out.push(Line::raw(""));
        out.push(Line::styled(text.side_usage.clone(), bold));
        let sum = meter::short(input + total.output);
        out.push(item(
            text.side_total.replace("{tokens}", &sum),
            theme::dim(),
        ));
        let split = text
            .side_split
            .replace("{input}", &meter::short(input))
            .replace("{output}", &meter::short(total.output));
        out.push(item(split, theme::dim()));
        let hit = meter::hit_rate(total.cache_read, input);
        out.push(item(text.side_hit.replace("{percent}", &hit), theme::dim()));
        if t.cache.breaks > 0 {
            let breaks = text.side_breaks.replace("{n}", &t.cache.breaks.to_string());
            out.push(item(breaks, theme::dim()));
        }
    }
    (out, id_row)
}

/// 上下文那根进度条：用了的强调色、没用的暗；压缩线落在的那一格换成警示色（黄），字不变（2026-09-30 项目主人定）。
/// `line` 是核心给的压缩线，没给的不标。
pub(super) fn bar(
    used: u64,
    window: u64,
    line: Option<u64>,
    room: usize,
    marks: &crate::config::Bar,
) -> Line<'static> {
    // 用过就至少亮一格，不然用得少时看不出有这根条。
    let full = if window == 0 || used == 0 {
        0
    } else {
        (((used as f64 / window as f64) * room as f64).round() as usize).max(1)
    }
    .min(room);
    let mark = line
        .filter(|_| window > 0 && room > 0)
        .map(|l| (((l as f64 / window as f64) * room as f64).floor() as usize).min(room - 1));
    let mut spans = vec![Span::raw("  ")];
    for i in 0..room {
        let (glyph, style) = if i < full {
            (&marks.full, theme::accent())
        } else {
            (&marks.empty, theme::dim())
        };
        let style = if mark == Some(i) {
            theme::warn()
        } else {
            style
        };
        spans.push(Span::styled(glyph.clone(), style));
    }
    Line::from(spans)
}

/// 路径折行：一行尽量多放，断在这一行最后一个 `/` 后面；一行里没有 `/`（一级目录本身就放不下）的按字折。
pub(super) fn wrap_path(path: &str, room: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut rest: Vec<char> = path.chars().collect();
    while !rest.is_empty() {
        // 放得进这一行的有几个字。
        let mut used = 0;
        let mut fit = 0;
        for c in &rest {
            let w = c.width().unwrap_or(0);
            if used + w > room {
                break;
            }
            used += w;
            fit += 1;
        }
        if fit == rest.len() {
            lines.push(rest.iter().collect());
            break;
        }
        // 放不完：断在最后一个 `/` 后面（打头的那个不算，不然留下一个孤零零的 `/`）。
        let cut = rest[..fit]
            .iter()
            .rposition(|c| *c == '/')
            .filter(|at| *at > 0)
            .map_or(fit.max(1), |at| at + 1);
        lines.push(rest[..cut].iter().collect());
        rest.drain(..cut);
    }
    lines
}

/// 一段：第一行是标题（蓝色），别的行缩进两格。
pub fn section(lines: Vec<Line<'static>>) -> Vec<Line<'static>> {
    lines
        .into_iter()
        .enumerate()
        .map(|(i, line)| {
            if i == 0 {
                line.style(theme::side_title())
            } else {
                let mut spans = vec![Span::raw("  ")];
                spans.extend(line.spans);
                Line::from(spans).style(line.style)
            }
        })
        .collect()
}

/// 画侧边栏：竖线、吉祥物（开着的话）、会话那一段、待办，一段之间空一行。
pub fn draw(frame: &mut Frame, area: Rect, app: &mut App) {
    if area.width == 0 {
        return;
    }
    let line: Vec<Line> = (0..area.height)
        .map(|_| Line::styled("│", theme::dim()))
        .collect();
    frame.render_widget(
        Paragraph::new(line),
        Rect::new(area.x - 1, area.y, 1, area.height),
    );
    let inner = area.width.saturating_sub(2);
    let mut top = area.y + 1;
    if app.config.layout.mascot_sidebar {
        let look = &app.config.mascot;
        let cols = look.cols.min(area.width);
        let mascot =
            Rect::new(area.x + (area.width - cols) / 2, top, cols, look.rows).intersection(area);
        super::mascot_view::draw(frame, mascot, app, true);
        top = mascot.bottom() + 1;
    }
    let total = app.usage_total();
    let (info, id_row) = info_lines(&app.transcript, &total, &app.config, inner, &app.cwd);
    let tall = u16::try_from(info.len()).unwrap_or(u16::MAX);
    let at = Rect::new(area.x + 1, top, inner, tall).intersection(area);
    // 写了字的行记下来，给侧边栏的选字用（第 7 条）。
    let mut text_rows: Vec<TextRow> = info
        .iter()
        .enumerate()
        .map(|(i, line)| TextRow {
            y: at.y + u16::try_from(i).unwrap_or(0),
            x: at.x,
            text: line.to_string(),
        })
        .collect();
    frame.render_widget(Paragraph::new(info), at);
    // 点短编号复制完整编号。
    app.areas.session_id = id_row.map_or(Rect::default(), |row| {
        let y = at.y + u16::try_from(row).unwrap_or(0);
        Rect::new(at.x, y, inner, 1).intersection(at)
    });
    // 待办：一段，标题加粗、项目缩进两格；侧边栏剩下多高就露多少行。这一块能点，点了展开全部、再点收起（第 4 条）。
    let top = at.bottom() + 1;
    let below = Rect::new(area.x + 1, top, inner, area.bottom().saturating_sub(top));
    let rows = usize::from(below.height.saturating_sub(1));
    let todo = section(todo_lines(
        &app.board,
        &app.config,
        below.width.saturating_sub(2),
        rows,
        app.todo_full,
    ));
    let tall = u16::try_from(todo.len())
        .unwrap_or(u16::MAX)
        .min(below.height);
    app.areas.todo = Rect {
        height: tall,
        ..below
    };
    text_rows.extend(todo.iter().enumerate().map(|(i, line)| TextRow {
        y: below.y + u16::try_from(i).unwrap_or(0),
        x: below.x,
        text: line.to_string(),
    }));
    frame.render_widget(Paragraph::new(todo), below);
    // 选中的反色。
    for row in &text_rows {
        if let Some((from, to)) = app.side_select.cols(row.y) {
            let cells = Rect::new(from, row.y, to - from, 1).intersection(area);
            frame
                .buffer_mut()
                .set_style(cells, Style::new().add_modifier(Modifier::REVERSED));
        }
    }
    app.side_select.set_rows(text_rows);
}

#[cfg(test)]
mod tests;
