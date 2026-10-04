//! 全屏供应商和模型配置页。只负责绘制与命中区域，读写由配置状态的 IPC 驱动。
mod editor;
mod lists;
#[cfg(test)]
mod tests;

use crate::{
    caret::Caret,
    config::Config,
    settings::{Hit, Settings, forms::Entity, text},
    theme,
};
use ratatui::crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::{Position, Rect},
    text::Line,
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};
use unicode_width::UnicodeWidthStr;

/// 配置页一帧；返回页内光标，不使用聊天输入框遗留位置。
pub fn draw(frame: &mut Frame, page: &mut Settings, config: &Config) -> Caret {
    page.hits.clear();
    let area = frame.area();
    frame.render_widget(Clear, area);
    let mut caret = Caret {
        at: Position::new(area.x, area.y),
        shown: false,
    };
    if area.width < 2 || area.height < 3 {
        return caret;
    }
    let gap = super::margins::margins(&config.layout, area.width)
        .side_gap
        .min(area.width / 4);
    let outer = Rect::new(
        area.x + gap,
        area.y,
        area.width.saturating_sub(gap * 2),
        area.height,
    );
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::dim())
        .title(text(&config.text.settings, "title"));
    let inner = block.inner(outer);
    frame.render_widget(block, outer);
    if inner.is_empty() {
        return caret;
    }
    caret.at = Position::new(inner.x, inner.y);
    let tab_rows = tabs(frame, page, config, inner);
    let body = Rect::new(
        inner.x,
        inner.y.saturating_add(tab_rows + 1),
        inner.width,
        inner.height.saturating_sub(tab_rows + 4),
    );
    if let Some(form) = &page.form {
        let title = if form.title.is_empty() {
            text(
                &config.text.settings,
                if matches!(form.entity, Entity::Pool { .. }) {
                    "add_pool"
                } else {
                    "add_provider"
                },
            )
        } else {
            &form.title
        };
        line(
            frame,
            Rect::new(inner.x, inner.y + tab_rows, inner.width, 1),
            title,
            theme::picked(),
        );
        lists::form(frame, page, config, body);
    } else {
        lists::draw(frame, page, config, body);
    }
    let status = Rect::new(inner.x, inner.bottom().saturating_sub(3), inner.width, 1);
    let note = if !page.note.is_empty() {
        page.note.as_str()
    } else if !page.connected {
        text(&config.text.settings, "not_connected")
    } else if !page.loaded {
        text(&config.text.settings, "loading")
    } else {
        ""
    };
    line(frame, status, note, theme::dim());
    footer(
        frame,
        page,
        config,
        Rect::new(inner.x, inner.bottom().saturating_sub(2), inner.width, 2),
    );
    if page.editing.is_some() && !body.is_empty() {
        caret = editor::draw(frame, page, config, body);
    }
    caret
}

fn tabs(frame: &mut Frame, page: &mut Settings, config: &Config, area: Rect) -> u16 {
    let keys = ["tab_providers", "tab_chat", "tab_vision", "tab_pools"];
    let needed: usize = keys
        .iter()
        .map(|k| text(&config.text.settings, k).width() + 4)
        .sum();
    let columns = if usize::from(area.width) >= needed {
        4u16
    } else {
        2u16
    };
    let rows = (4 / columns).min(area.height);
    for (i, key) in keys.into_iter().enumerate() {
        let col = i as u16 % columns;
        let row = i as u16 / columns;
        if row >= rows {
            continue;
        }
        let x = area.x + area.width * col / columns;
        let end = area.x + area.width * (col + 1) / columns;
        let hit = Rect::new(x, area.y + row, end - x, 1);
        let title = format!(" {} ", text(&config.text.settings, key));
        line(
            frame,
            hit,
            &title,
            if page.tab == i {
                theme::picked()
            } else {
                theme::dim()
            },
        );
        page.hits.push((hit, Hit::Tab(i)));
    }
    rows
}

fn footer(frame: &mut Frame, page: &mut Settings, config: &Config, area: Rect) {
    if area.is_empty() {
        return;
    }
    let key = if page.editing.is_some() {
        "edit_hint"
    } else if page
        .form
        .as_ref()
        .is_some_and(|f| matches!(f.entity, Entity::Pool { .. }))
    {
        "pool_hint"
    } else if page.form.is_some() {
        "form_hint"
    } else if page.tab == 0 {
        "provider_hint"
    } else {
        "list_hint"
    };
    line(
        frame,
        Rect::new(area.x, area.y, area.width, 1),
        text(&config.text.settings, key),
        theme::dim(),
    );
    if area.height < 2 {
        return;
    }
    let actions = if page.editing.is_some() {
        vec![
            (KeyCode::Enter, "Enter", "confirm"),
            (KeyCode::Esc, "Esc", "cancel"),
        ]
    } else if page.form.is_some() {
        vec![
            (KeyCode::Char('s'), "s", "save"),
            (KeyCode::Esc, "Esc", "cancel"),
        ]
    } else {
        vec![(KeyCode::Esc, "Esc", "cancel")]
    };
    let mut x = area.x;
    for (code, key, label) in actions {
        let title = format!("{key} {}  ", text(&config.text.settings, label));
        let width = u16::try_from(title.width())
            .unwrap_or(u16::MAX)
            .min(area.right().saturating_sub(x));
        let hit = Rect::new(x, area.y + 1, width, 1);
        line(frame, hit, &title, theme::dim());
        page.hits.push((hit, Hit::Action(code)));
        x = x.saturating_add(width);
    }
}

/// 一行由 ratatui 裁剪，不发送终端控制序列。
fn line(frame: &mut Frame, area: Rect, value: &str, style: ratatui::style::Style) {
    if !area.is_empty() {
        frame.render_widget(Paragraph::new(Line::styled(value.to_string(), style)), area);
    }
}

/// 选择行跟随视口，保留最少需要的滚动。
fn top(selected: usize, count: usize, shown: usize) -> usize {
    selected
        .saturating_sub(shown.saturating_sub(1))
        .min(count.saturating_sub(shown))
}

/// 绘制可点列表行；只有当前焦点使用选中样式。
fn row(frame: &mut Frame, page: &mut Settings, area: Rect, value: &str, selected: bool, hit: Hit) {
    line(
        frame,
        area,
        value,
        if selected {
            theme::selected()
        } else {
            ratatui::style::Style::default()
        },
    );
    if !area.is_empty() {
        page.hits.push((area, hit));
    }
}
