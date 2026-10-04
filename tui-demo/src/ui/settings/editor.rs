//! 字段编辑器：密钥在缓冲区只画掩码，长输入按字素水平裁剪，光标放在页内。
use super::{line, row, top};
use crate::{
    caret::Caret,
    config::Config,
    settings::{Hit, Settings, text},
    theme,
};
use ratatui::{
    Frame,
    layout::{Position, Rect},
    widgets::{Block, BorderType, Borders, Clear},
};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

pub(super) fn draw(frame: &mut Frame, page: &mut Settings, config: &Config, area: Rect) -> Caret {
    let mut caret = Caret {
        at: Position::new(area.x, area.y),
        shown: false,
    };
    if area.width < 3 || area.height < 3 {
        return caret;
    }
    let Some(edit) = &page.editing else {
        return caret;
    };
    let title = page
        .form
        .as_ref()
        .and_then(|f| f.fields.get(edit.field))
        .map(|f| text(&config.text.settings, &f.label))
        .unwrap_or("");
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme::picked())
        .title(title);
    let inside = block.inner(area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    caret.at = Position::new(inside.x, inside.y);
    if inside.is_empty() {
        return caret;
    }
    if !edit.options.is_empty() {
        let options_area = if edit.checked.is_some() && inside.height > 1 {
            line(
                frame,
                Rect::new(inside.x, inside.bottom() - 1, inside.width, 1),
                text(&config.text.settings, "inputs_hint"),
                theme::dim(),
            );
            Rect::new(inside.x, inside.y, inside.width, inside.height - 1)
        } else {
            inside
        };
        let options: Vec<_> = edit
            .options
            .iter()
            .enumerate()
            .map(|(i, o)| {
                let value = if o.is_empty() {
                    text(&config.text.settings, "automatic")
                } else {
                    text(&config.text.settings, o)
                };
                let mark = edit.checked.as_ref().map_or(String::new(), |v| {
                    format!("[{}] ", if v.get(i) == Some(&true) { "✓" } else { " " })
                });
                (format!("{mark}{value}"), i == edit.selected)
            })
            .collect();
        let start = top(
            edit.selected,
            options.len(),
            usize::from(options_area.height),
        );
        for (offset, (i, (value, selected))) in options
            .iter()
            .enumerate()
            .skip(start)
            .take(usize::from(options_area.height))
            .enumerate()
        {
            row(
                frame,
                page,
                Rect::new(inside.x, inside.y + offset as u16, inside.width, 1),
                value,
                *selected,
                Hit::Option(i),
            );
            if *selected {
                caret.at = Position::new(inside.x, inside.y + offset as u16);
            }
        }
        return caret;
    }
    let (value, column) = viewport(
        edit.editor.text(),
        edit.editor.cursor(),
        edit.secret,
        usize::from(inside.width),
    );
    line(
        frame,
        Rect::new(inside.x, inside.y, inside.width, 1),
        &value,
        ratatui::style::Style::default(),
    );
    if inside.height > 1 {
        line(
            frame,
            Rect::new(inside.x, inside.y + 1, inside.width, 1),
            text(
                &config.text.settings,
                if edit.secret {
                    "secret_hint"
                } else {
                    "edit_hint"
                },
            ),
            theme::dim(),
        );
    }
    caret = Caret {
        at: Position::new(inside.x + column as u16, inside.y),
        shown: true,
    };
    frame.set_cursor_position(caret.at);
    caret
}

/// 字节光标转换为显示列；留一格给行尾光标，不切开中文或组合字素。
fn viewport(value: &str, cursor: usize, secret: bool, width: usize) -> (String, usize) {
    let parts: Vec<_> = value
        .grapheme_indices(true)
        .map(|(at, g)| {
            (
                at,
                if secret {
                    "•"
                } else if g == "\n" || g == "\r" {
                    " "
                } else {
                    g
                },
            )
        })
        .collect();
    let before = parts.iter().take_while(|(at, _)| *at < cursor).count();
    let available = width.saturating_sub(1);
    let mut start = 0;
    let mut columns = parts[..before]
        .iter()
        .map(|(_, g)| g.width())
        .sum::<usize>();
    while columns > available && start < before {
        columns = columns.saturating_sub(parts[start].1.width());
        start += 1;
    }
    let mut displayed = String::new();
    let mut used = 0;
    for (_, g) in parts.iter().skip(start) {
        if used + g.width() > available {
            break;
        }
        displayed.push_str(g);
        used += g.width();
    }
    (displayed, columns.min(available))
}

#[cfg(test)]
mod tests {
    use super::viewport;
    #[test]
    fn horizontal_input_clipping_preserves_unicode_and_masks_by_grapheme() {
        assert_eq!(viewport("abcdef", 6, false, 4), ("def".into(), 3));
        assert_eq!(viewport("中文abc", 9, false, 6), ("文abc".into(), 5));
        let (shown, at) = viewport("私密🔑", "私密🔑".len(), true, 4);
        assert_eq!(shown, "•••");
        assert_eq!(at, 3);
    }
}
