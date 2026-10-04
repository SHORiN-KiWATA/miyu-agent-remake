//! 列表和草稿行：命中编号保持核心列表顺序，滚动只改变显示位置。
use super::{line, row, top};
use crate::{
    config::Config,
    settings::{Hit, Settings, forms::Entity, text},
    theme,
};
use ratatui::{Frame, layout::Rect};
use unicode_width::UnicodeWidthStr;

pub(super) fn draw(frame: &mut Frame, page: &mut Settings, config: &Config, area: Rect) {
    if area.is_empty() {
        return;
    }
    if page.tab == 0 {
        providers(frame, page, config, area);
        return;
    }
    let rows: Vec<String> = if page.tab == 3 {
        page.pools
            .iter()
            .map(|p| {
                format!(
                    "{} · {} · {}",
                    p.name,
                    p.strategy,
                    if p.models.is_empty() {
                        text(&config.text.settings, "empty_pool").to_string()
                    } else {
                        p.models.len().to_string()
                    }
                )
            })
            .collect()
    } else {
        let current = page.uses[if page.tab == 1 { "chat" } else { "vision" }].as_str();
        page.models()
            .into_iter()
            .map(|(_, m)| {
                format!(
                    "{} {}",
                    if current == Some(m.reference.as_str()) {
                        "●"
                    } else {
                        " "
                    },
                    m.reference
                )
            })
            .collect()
    };
    list(frame, page, config, area, &rows, page.selected, false);
}

fn providers(frame: &mut Frame, page: &mut Settings, config: &Config, area: Rect) {
    let left_width = (area.width / 3).max(1);
    let left = Rect::new(area.x, area.y, left_width, area.height);
    let right = Rect::new(
        left.right().saturating_add(1),
        area.y,
        area.width.saturating_sub(left_width + 1),
        area.height,
    );
    line(
        frame,
        Rect::new(left.x, left.y, left.width, 1),
        text(&config.text.settings, "providers"),
        theme::dim(),
    );
    line(
        frame,
        Rect::new(right.x, right.y, right.width, 1),
        text(&config.text.settings, "models"),
        theme::dim(),
    );
    let left = Rect::new(
        left.x,
        left.y + 1,
        left.width,
        left.height.saturating_sub(1),
    );
    let right = Rect::new(
        right.x,
        right.y + 1,
        right.width,
        right.height.saturating_sub(1),
    );
    let names: Vec<_> = page.providers.iter().map(|p| p.name.clone()).collect();
    list(frame, page, config, left, &names, page.provider, true);
    let models: Vec<_> = page
        .providers
        .get(page.provider)
        .map_or_else(Vec::new, |p| {
            p.models.iter().map(|m| m.id.clone()).collect()
        });
    if models.is_empty() {
        line(
            frame,
            right,
            text(&config.text.settings, "empty"),
            theme::dim(),
        );
        return;
    }
    let model_width = models
        .iter()
        .map(|name| name.width())
        .max()
        .unwrap_or(1)
        .min(config.layout.session_title_width.max(1));
    let columns = if area.width >= config.layout.compact_below
        && usize::from(right.width) >= model_width.saturating_mul(2).saturating_add(1)
        && models.len() > usize::from(right.height)
    {
        2usize
    } else {
        1
    };
    let height = usize::from(right.height);
    let start = top(page.selected, models.len(), height * columns);
    for col in 0..columns {
        let width = right.width / u16::try_from(columns).unwrap_or(1);
        for r in 0..height {
            let i = start + col * height + r;
            let Some(value) = models.get(i) else {
                continue;
            };
            let hit = Rect::new(right.x + width * col as u16, right.y + r as u16, width, 1);
            row(
                frame,
                page,
                hit,
                value,
                page.column == 1 && i == page.selected,
                Hit::Row(i),
            );
        }
    }
}

fn list(
    frame: &mut Frame,
    page: &mut Settings,
    config: &Config,
    area: Rect,
    values: &[String],
    selected: usize,
    provider: bool,
) {
    if values.is_empty() {
        line(
            frame,
            area,
            text(&config.text.settings, "empty"),
            theme::dim(),
        );
        return;
    }
    let start = top(selected, values.len(), usize::from(area.height));
    for (offset, (i, value)) in values
        .iter()
        .enumerate()
        .skip(start)
        .take(usize::from(area.height))
        .enumerate()
    {
        let hit = Rect::new(area.x, area.y + offset as u16, area.width, 1);
        row(
            frame,
            page,
            hit,
            value,
            i == selected && (!provider || page.column == 0),
            if provider {
                Hit::Provider(i)
            } else {
                Hit::Row(i)
            },
        );
    }
}

pub(super) fn form(frame: &mut Frame, page: &mut Settings, config: &Config, area: Rect) {
    let Some(form) = &page.form else {
        return;
    };
    let is_model = matches!(form.entity, Entity::Model);
    let mut values: Vec<String> = form
        .fields
        .iter()
        .map(|f| {
            let label = text(&config.text.settings, &f.label);
            let shown = if f.value.is_empty() {
                text(&config.text.settings, "automatic")
            } else {
                f.shown()
            };
            let source = if f.source.is_empty() {
                String::new()
            } else {
                format!(" · {}", text(&config.text.settings, &f.source))
            };
            let value = if f.label == "inputs" {
                serde_json::from_str::<Vec<String>>(&f.value)
                    .map(|tags| {
                        tags.iter()
                            .map(|tag| format!("[{}]", text(&config.text.settings, tag)))
                            .collect::<Vec<_>>()
                            .join(" ")
                    })
                    .unwrap_or_else(|_| shown.to_string())
            } else {
                shown.to_string()
            };
            format!("{label}  {value}{source}")
        })
        .collect();
    if let Entity::Pool { members, .. } = &form.entity {
        values.extend(page.models().into_iter().map(|(_, m)| {
            format!(
                "[{}] {}",
                if members.contains(&m.reference) {
                    "✓"
                } else {
                    " "
                },
                m.reference
            )
        }));
    }
    let mut rows_area = area;
    if is_model && area.height > 0 {
        line(
            frame,
            Rect::new(area.x, area.y, area.width, 1),
            text(&config.text.settings, "model_price_unit"),
            theme::dim(),
        );
        rows_area.y += 1;
        rows_area.height -= 1;
    }
    list(
        frame,
        page,
        config,
        rows_area,
        &values,
        page.selected,
        false,
    );
}
