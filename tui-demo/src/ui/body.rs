//! 正文区：把排好的行放进视口，铺上点开的底色和选区的反色。
//!
//! 放得下时从顶上开始；放不下时露出最新的那一截，滚过以后钉在滚到的地方（`body_view.rs`）。

use crate::ui::rows::Target;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use unicode_width::UnicodeWidthStr;

use super::Areas;
use super::row_cache::{self, Rows};
use super::rows::Ctx;
use super::timeline;
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
        // 图最多占窗口高度的几分之几：照窗口的高，不照正文区（开关列表时正文区变矮，照它算每次都要重做）。
        screen_rows: frame.area().height,
        md: &app.md_cache,
        figures: &app.figures,
        cards: &app.cards,
        diagrams: &app.diagrams,
        writing: app.transcript.writing().map(|e| e.id),
        frame: usize::try_from(app.started.elapsed().as_millis() / u128::from(spinner_ms))
            .unwrap_or(0),
    };
    // 整份重排时一帧只排预算这么多，视口附近先排（蓝图「正文」第 8 条）：翻上去看着的照上一帧视口顶上那一条。
    let plan = row_cache::Plan {
        budget: Some(std::time::Duration::from_millis(
            app.config.layout.relayout_budget_ms,
        )),
        anchor: app
            .view
            .top
            .and_then(|_| app.view.rows.entry_at(app.view.first)),
    };
    let mut rows = crate::frame_log::section("rows", || {
        row_cache::build(&app.transcript.entries, &ctx, &app.row_cache, plan)
    });
    // 等她的第一个字：正文末尾先转着（`tui.md`「时间线」第 19 条）。
    if app.transcript.waiting() {
        rows.push(timeline::tail_rows(&ctx).into());
    }
    // 还算在看的那段回答：长过视口时停在它的开头（`tui.md`「正文」第 1 条）。在写的那段一律算；写完了的（后面只有收尾
    // 那一行），上一帧开头还露着才算：后到的图、卡片把它撑高时停住，一口气整段到的、补发来的照旧跟着最新的。
    let latest = app.transcript.reading().and_then(|(i, writing)| {
        let entry = &app.transcript.entries[i];
        let start = rows.start_of(i).filter(|_| super::rows::shown(entry))?;
        Some((entry.id, start, writing))
    });
    app.view.reading = still_reading(&app.view, latest);
    app.view.latest = latest.map(|(id, start, _)| (id, start));
    let settled = app.row_cache.borrow().stale == 0;
    let first = first_row(&rows, area, &mut app.view, settled);
    let height = usize::from(area.height);
    let indent = u16::try_from(ctx.indent.width()).unwrap_or(0);
    let shade = shade_span(area, indent, ctx.width);
    for (i, row) in rows.window(first, height) {
        let y = area.y + u16::try_from(i - first).unwrap_or(0);
        // 先铺底色再写字：没带底色的片段留着底下的灰，差异行自己的红底、青底盖在上面。
        if row.shade {
            let cells = Rect::new(shade.0, y, shade.1, 1).intersection(area);
            frame.buffer_mut().set_style(cells, theme::shade());
        }
        frame
            .buffer_mut()
            .set_line(area.x, y, &row.line, area.width);
    }
    app.view.first = first;
    crate::frame_log::section("figures", || {
        super::figure_rows::draw(
            frame.buffer_mut(),
            area,
            &rows,
            first,
            &mut app.figures.borrow_mut(),
        );
    });
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
                // 图占着的格子不画：下划线会叠在图上（链接卡片的封面图，2026-10-02 项目主人报）。
                let buf = frame.buffer_mut();
                for cx in cells.left()..cells.right() {
                    let cell = &mut buf[(cx, y)];
                    if !super::figure_rows::is_picture(cell) {
                        cell.set_style(underline);
                    }
                }
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

/// 最新的那段回答（编号、第一行、还在不在写）算不算在看：在写的算；写完了的，上一帧（`view.latest`、`view.first`）
/// 它的开头还露着才算（`tui.md`「正文」第 1 条）。
fn still_reading(
    view: &crate::body_view::BodyView,
    latest: Option<(u64, usize, bool)>,
) -> Option<(u64, usize)> {
    let (id, start, writing) = latest?;
    let seen = view
        .latest
        .is_some_and(|(was, top)| was == id && top >= view.first);
    (writing || seen).then_some((id, start))
}

/// 点开的一块铺底色的范围（从第几列起、多宽）：左边和你说的话前面的 `┃` 同一列（缩进 `indent` 以后那两格槽），
/// 右边比字（`text_width` 列）宽出同样两格（`tui.md`「时间线」第 14 条）。
fn shade_span(area: Rect, indent: u16, text_width: u16) -> (u16, u16) {
    (area.x + indent, text_width + 4)
}

/// 点开的是 `clicked`：`row` 这一行算不算它展开的那一块。点一条（撤销那一行）时，它里面能单独点开的小块（改回的文件，
/// `Details`）也算这一块。
fn same_block(row: Target, clicked: Target) -> bool {
    match (row, clicked) {
        (Target::Details(a, _), Target::Entry(b)) => a == b,
        _ => row == clicked,
    }
}

/// 上一帧的第 `row` 行在这一帧是第几行：照它是哪一条的第几行找，那一条短了的停在它的最后一行。
fn remap(before: &Rows, now: &Rows, row: usize) -> usize {
    let Some(entry) = before.entry_at(row) else {
        return row.min(now.len().saturating_sub(1));
    };
    let offset = row.saturating_sub(before.start_of(entry).unwrap_or(row));
    match (now.start_of(entry), now.end_of(entry)) {
        (Some(start), Some(end)) => (start + offset).min(end.saturating_sub(1)),
        _ => row.min(now.len().saturating_sub(1)),
    }
}

/// 第一行露出的是第几行：有锚点的照锚点，滚过的照滚到的，别的跟着最新的、只往下走。滚到底了就回到跟着最新的。
/// 顺手记下这一帧的区域（`view.area`），下一帧照它看窗口宽度变没变。
fn first_row(
    rows: &Rows,
    area: Rect,
    view: &mut crate::body_view::BodyView,
    settled: bool,
) -> usize {
    let height = usize::from(area.height);
    let bottom = rows.len().saturating_sub(height);
    if let Some((target, y)) = view.anchor.take()
        && let Some(i) = rows.iter().position(|r| r.target == Some(target))
    {
        let mut top = i.saturating_sub(usize::from(y.saturating_sub(area.y)));
        // 展开的内容长到视口外面了：往上推到露出来，最多推到被点的那一行顶到最上面（2026-10-02 项目主人报：
        // 最底下那一行点开以后什么都看不见）。
        let end = i + rows
            .iter()
            .skip(i)
            .take_while(|r| r.target.is_some_and(|t| same_block(t, target)))
            .count();
        if end > top + height {
            top = (end - height).min(i);
        }
        view.top = Some(top);
        view.auto = false;
    }
    // 窗口变宽变窄、或者整份重排还在分帧做（没排到的先用旧行）：行号一帧一个样，上一帧记的对不上了，不守它
    // （`tui.md`「正文」第 1 条；2026-10-02 项目主人报：最大化以后闪一下就只剩最后一行）。
    let relaid = view.area.width != area.width || !settled;
    if relaid {
        view.floor_end = 0;
    }
    // 清屏那一行、滚到的地方也是照行号记的：换成「在哪一条的第几行」，照这一帧的行找回来，不然变宽以后落在内容
    // 后面，正文全空（同一天项目主人报）。
    if !view.rows.is_empty() {
        if let Some(at) = view.cleared_at.filter(|&at| at > 0) {
            let moved = view.rows.entry_at(at - 1).and_then(|e| rows.end_of(e));
            view.cleared_at = Some(moved.unwrap_or(at).min(rows.len()));
        }
        if relaid && let Some(top) = view.top {
            view.top = Some(remap(&view.rows, rows, top));
        }
    }
    // 一轮结束按住了：视口长高的那几行记进底边，第一行不往回退（`view.hold`）。
    if std::mem::take(&mut view.hold) && view.floor_end > 0 {
        view.floor_end += usize::from(area.height.saturating_sub(view.area.height));
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
            let follow = lowest.max(view.floor_end.saturating_sub(height)).min(cap);
            // 她正在写的一段正文长过视口：第一行顶到最上面就停住，一段只停一次（`tui.md`「正文」第 1 条）。
            match view.reading {
                Some((id, start)) if follow > start && view.paused != Some(id) => {
                    view.paused = Some(id);
                    view.top = Some(start);
                    view.auto = true;
                    start
                }
                _ => follow,
            }
        }
    };
    if first >= lowest {
        view.top = None;
        view.auto = false;
    }
    if view.top.is_none() {
        view.seen_end = rows.len();
    }
    // 内容还放得下（从第一行露起；清过屏的，从清屏那一行露起）时不记：视口变矮时不把开头的行挤出去
    // （`tui.md`「正文」第 1 条、「按键」Ctrl+L）。
    let top = view.cleared_at.map_or(0, |at| at.min(rows.len()));
    view.floor_end = if first == top || !settled {
        0
    } else {
        first + height
    };
    view.area = area;
    first
}

#[cfg(test)]
mod tests;
