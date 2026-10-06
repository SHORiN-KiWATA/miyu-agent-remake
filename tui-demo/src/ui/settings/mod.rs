//! 画配置页（蓝图 `tui.md`「配置页」第 3 到 7 条）：整块居中占八成、窄屏用满宽，不画边框；面包屑、分页、几栏、状态行、
//! 细线、两行按键提示；悬浮窗盖在上面，背后调暗。只画，不改数据（选中行滚到哪、鼠标点得到哪记在页面上）。

mod columns;
mod fields;
mod popup;

use ratatui::Frame;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use serde::Deserialize;
use unicode_width::UnicodeWidthStr;

use crate::caret::Caret;
use crate::config::Config;
use crate::settings::nav::Page;
use crate::settings::{Hit, Settings, Texts, Tone};
use crate::theme;

/// 配置页的尺寸（`layout.json` 的 `settings`）。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Look {
    /// 宽占终端的百分之几。
    pub width_percent: u16,
    /// 高占终端的百分之几。
    pub height_percent: u16,
    /// 终端窄于这么多列时用满宽。
    pub narrow_below: u16,
    /// 供应商栏多宽。
    pub provider_width: u16,
    /// 组织栏多宽。
    pub org_width: u16,
    /// 用途栏多宽。
    pub use_width: u16,
    /// 池栏多宽。
    pub pool_width: u16,
    /// 栏和栏之间空几列。
    pub column_gap: u16,
    /// 悬浮窗多宽。
    pub popup_width: u16,
    /// 宽一些的悬浮窗（模型、池、选模型）多宽。
    pub popup_wide: u16,
    /// 悬浮窗最高占终端的百分之几。
    pub popup_height_percent: u16,
}

/// 画一帧配置页。
pub fn draw(frame: &mut Frame, page: &mut Settings, config: &Config, caret: &mut Caret) {
    let look = &config.layout.settings;
    let texts = &config.text.settings;
    page.hits.clear();
    let area = frame.area();
    let stage = stage(area, look);
    if page.on_menu {
        menu(frame, stage, page, texts);
    } else {
        models(frame, stage, page, texts, look, caret);
    }
    if page.popup.is_some() {
        let buf = frame.buffer_mut();
        buf.set_style(area, Style::new().add_modifier(Modifier::DIM));
        popup::draw(frame, area, page, texts, look, caret);
    }
}

/// 整块放在哪：宽、高各占八成居中；窄屏用满宽，左右留一列。
fn stage(area: Rect, look: &Look) -> Rect {
    if area.width < look.narrow_below {
        let x = area.x + area.width.min(1);
        let height = area.height.saturating_sub(2);
        return Rect::new(
            x,
            area.y + area.height.min(1),
            area.width.saturating_sub(2),
            height,
        );
    }
    let width = area.width * look.width_percent / 100;
    let height = area.height * look.height_percent / 100;
    Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    )
}

/// 主菜单：面包屑、一项、状态行、细线、按键提示。
fn menu(frame: &mut Frame, stage: Rect, page: &mut Settings, texts: &Texts) {
    crumb(frame, stage, texts, false);
    if stage.height > 4 {
        let row = Rect::new(
            stage.x,
            stage.y + 2,
            stage.width.min(46),
            2.min(stage.height - 2),
        );
        let buf = frame.buffer_mut();
        buf.set_style(row, theme::row_focus());
        bar(frame, row);
        let name = Line::from(Span::raw(texts.entry.as_str()));
        frame
            .buffer_mut()
            .set_line(row.x + 2, row.y, &name, row.width.saturating_sub(2));
        if row.height > 1 {
            let note = Line::from(Span::styled(texts.entry_note.as_str(), theme::dim()));
            frame
                .buffer_mut()
                .set_line(row.x + 2, row.y + 1, &note, row.width.saturating_sub(2));
        }
        page.hits.push((row, Hit::Menu));
    }
    footer(frame, stage, page, texts, &texts.key_hints[0], None, None);
}

/// 「供应商和模型」：面包屑、分页、几栏、状态行、细线、两行按键提示。
fn models(
    frame: &mut Frame,
    stage: Rect,
    page: &mut Settings,
    texts: &Texts,
    look: &Look,
    caret: &mut Caret,
) {
    crumb(frame, stage, texts, true);
    if stage.height < 8 {
        return;
    }
    tabs(
        frame,
        Rect::new(stage.x, stage.y + 2, stage.width, 1),
        page,
        texts,
    );
    let body = Rect::new(
        stage.x,
        stage.y + 4,
        stage.width,
        stage.height.saturating_sub(8),
    );
    columns::draw(frame, body, page, texts, look);
    let hints = match page.nav.page {
        Page::Providers => &texts.key_hints[1],
        Page::Defaults => &texts.key_hints[2],
        Page::Pools => &texts.key_hints[3],
    };
    let col = page.nav.focus(&page.view);
    let len = page.nav.len(col, &page.view);
    let counter = format!(
        "{}/{len}",
        if len == 0 {
            0
        } else {
            page.nav.selected(col) + 1
        }
    );
    footer(
        frame,
        stage,
        page,
        texts,
        hints,
        Some(&texts.key_hints[4]),
        Some(counter),
    );
    if let Some(search) = page.nav.search.as_ref().filter(|s| s.typing) {
        let y = stage.bottom().saturating_sub(4);
        let x = stage.x + 2 + search.text.width() as u16;
        caret.put(
            Position::new(x.min(stage.right().saturating_sub(1)), y),
            true,
        );
    }
}

/// 面包屑：`● 配置`，进了一页再接 ` — ◉ 供应商和模型`。
fn crumb(frame: &mut Frame, stage: Rect, texts: &Texts, inside: bool) {
    let mut spans = vec![Span::styled("● ", theme::accent())];
    if inside {
        spans.push(Span::styled(texts.title.as_str(), theme::dim()));
        spans.push(Span::styled(" — ", theme::dim()));
        spans.push(Span::styled("◉ ", theme::accent()));
        spans.push(Span::styled(
            texts.entry.as_str(),
            theme::accent().add_modifier(Modifier::BOLD),
        ));
    } else {
        spans.push(Span::styled(
            texts.title.as_str(),
            theme::accent().add_modifier(Modifier::BOLD),
        ));
    }
    frame
        .buffer_mut()
        .set_line(stage.x, stage.y, &Line::from(spans), stage.width);
}

/// 分页：当前页加粗、下面一道强调色（终端里画成下划线）；鼠标点得动。
fn tabs(frame: &mut Frame, row: Rect, page: &mut Settings, texts: &Texts) {
    let mut x = row.x;
    let current = crate::settings::nav::PAGES
        .iter()
        .position(|p| *p == page.nav.page);
    for (i, name) in texts.pages.iter().enumerate() {
        let on = current == Some(i);
        // 当前页铺强调色的底（2026-10-07 项目主人：只画下划线，三页看不出哪页是哪页）。
        let style = if on {
            theme::picked_bar().add_modifier(Modifier::BOLD)
        } else {
            theme::dim()
        };
        let label = format!(" {name} ");
        let width = label.width() as u16;
        if x + width > row.right() {
            break;
        }
        frame.buffer_mut().set_string(x, row.y, &label, style);
        page.hits.push((Rect::new(x, row.y, width, 1), Hit::Tab(i)));
        x += width;
        if i + 1 < texts.pages.len() && x + 3 <= row.right() {
            frame
                .buffer_mut()
                .set_string(x, row.y, " · ", theme::faint());
            x += 3;
        }
    }
}

/// 下面四行：状态行（右边写有几项没存）、细线、一两行按键提示（第二行右边写第几个）。
fn footer(
    frame: &mut Frame,
    stage: Rect,
    page: &Settings,
    texts: &Texts,
    first: &[[String; 2]],
    second: Option<&[[String; 2]]>,
    counter: Option<String>,
) {
    if stage.height < 4 {
        return;
    }
    let bottom = stage.bottom();
    let status_y = bottom - 4;
    let buf = frame.buffer_mut();
    let (text, style) = status(page, texts);
    buf.set_line(
        stage.x,
        status_y,
        &Line::from(vec![Span::styled(text, style)]),
        stage.width,
    );
    let rule = "─".repeat(stage.width as usize);
    buf.set_string(stage.x, bottom - 3, &rule, theme::faint());
    let (hint_y, second_y) = if second.is_some() {
        (bottom - 2, bottom - 1)
    } else {
        (bottom - 2, bottom - 2)
    };
    let search = page.nav.search.as_ref().is_some_and(|s| s.typing);
    let first = if search {
        texts.key_hints[5].as_slice()
    } else {
        first
    };
    buf.set_line(stage.x, hint_y, &hint_line(first), stage.width);
    if let Some(second) = second.filter(|_| !search) {
        buf.set_line(stage.x, second_y, &hint_line(second), stage.width);
    }
    if let Some(counter) = counter {
        let w = counter.width() as u16;
        if w < stage.width {
            buf.set_string(stage.right() - w, second_y, &counter, theme::dim());
        }
    }
}

/// 状态行左边写什么：在筛的写筛的字；有话的写话；没有的照这一页写模型、池有几个。
fn status(page: &Settings, texts: &Texts) -> (String, Style) {
    if let Some(search) = &page.nav.search {
        let text = format!("/ {}", search.text);
        return (
            text,
            if search.typing {
                Style::new()
            } else {
                theme::dim()
            },
        );
    }
    if let Some((text, tone)) = &page.status {
        let style = match tone {
            Tone::Note => theme::warn(),
            Tone::Good => theme::good(),
            Tone::Bad => theme::error(),
            Tone::Busy => theme::dim(),
        };
        return (text.clone(), style);
    }
    if page.loading() {
        return (texts.status("loading"), theme::dim());
    }
    if page.on_menu {
        return (String::new(), Style::new());
    }
    let text = match page.nav.page {
        Page::Providers => match page.nav.provider(&page.view).map(|p| p.models.len()) {
            Some(0) => texts.status("no_models"),
            Some(n) => texts.status("fetched").replace("{n}", &n.to_string()),
            None => String::new(),
        },
        Page::Pools => texts
            .status("pools")
            .replace("{n}", &page.view.pools.len().to_string()),
        Page::Defaults => String::new(),
    };
    (text, theme::warn())
}

/// 一行按键提示：键黄色，说明暗色，每格隔三列。
pub(super) fn hint_line(hints: &[[String; 2]]) -> Line<'_> {
    let mut spans = Vec::new();
    for (i, [key, what]) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("   "));
        }
        spans.push(Span::styled(key.as_str(), theme::warn()));
        spans.push(Span::styled(format!(" {what}"), theme::dim()));
    }
    Line::from(spans)
}

/// 选中的那一行左边的强调色竖条。
pub(super) fn bar(frame: &mut Frame, row: Rect) {
    for y in row.y..row.bottom() {
        frame
            .buffer_mut()
            .set_string(row.x, y, "┃", theme::accent());
    }
}

/// 字太长裁掉、末尾写「…」。
pub(super) fn fit(text: &str, width: usize) -> String {
    if text.width() <= width {
        return text.to_string();
    }
    let mut out = String::new();
    for c in text.chars() {
        if out.width() + unicode_width::UnicodeWidthChar::width(c).unwrap_or(0) + 1 > width {
            break;
        }
        out.push(c);
    }
    out.push('…');
    out
}

#[cfg(test)]
mod tests;
