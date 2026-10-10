//! 示范对话的两种窗（蓝图「配置页」第 41 条，照旧版的「预设对话」）：列表一轮两行（你问的、AI回的），选中的那一轮铺底
//! 色；两格窗各一行，光标在的那一格铺底色、跟着光标。

use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use super::fit;
use super::popup::{Body, Lines};
use crate::settings::Texts;
use crate::settings::pages::dialogs::{DialogsView, PairEdit};
use crate::theme;

/// 列表：一轮一行，两边各截前面一段；没有的写「还没有示范对话」。
pub(super) fn dialogs(lines: &mut Lines, view: &DialogsView, texts: &Texts, width: u16) {
    let words = &texts.more.dialogs;
    lines.title(words.title.clone(), String::new());
    lines.hints = texts.more.hints[8].clone();
    if view.pairs.is_empty() {
        lines.plain(Line::styled(format!("  {}", words.empty), theme::dim()));
        return;
    }
    // 一轮两行：你问的、AI回的，名字对齐；选中的那一轮两行都铺底色。
    let label_w = words.sides.iter().map(|s| s.width()).max().unwrap_or(0);
    let room = usize::from(width).saturating_sub(label_w + 8);
    for (i, pair) in view.pairs.iter().enumerate() {
        for (side, text) in words.sides.iter().zip([&pair.user, &pair.assistant]) {
            let pad = " ".repeat(label_w.saturating_sub(side.width()));
            lines.body.push(Body {
                line: Line::from(vec![
                    Span::styled(format!("  {side}{pad}  "), theme::dim()),
                    Span::raw(fit(text.lines().next().unwrap_or_default(), room)),
                ]),
                focused: i == view.sel,
                hit: None,
            });
        }
    }
}

/// 两格窗：user 一行、assistant 一行，光标在的那一格下划线；有一格空着按了回车的写红字。
pub(super) fn pair(lines: &mut Lines, edit: &PairEdit, texts: &Texts) {
    let words = &texts.more.dialogs;
    let title = if edit.index.is_some() {
        &words.edit
    } else {
        &words.add
    };
    lines.title(title.clone(), String::new());
    lines.hints = texts.more.hints[9].clone();
    let label_w = words.sides.iter().map(|s| s.width()).max().unwrap_or(0);
    for (i, (side, editor)) in words.sides.iter().zip(&edit.sides).enumerate() {
        let typed = editor.text();
        let pad = " ".repeat(label_w.saturating_sub(side.width()));
        let lead = format!("  {side}{pad}  ");
        // 光标在的那一格铺底色、跟着光标；字不加下划线（2026-10-08 项目主人）。
        if i == edit.focus {
            let before = &typed[..editor.cursor().min(typed.len())];
            let col = lead.width() + before.width();
            lines.caret = Some((lines.body.len(), u16::try_from(col).unwrap_or(0)));
        }
        lines.body.push(Body {
            line: Line::from(vec![
                Span::styled(lead, theme::dim()),
                Span::raw(typed.to_string()),
            ]),
            focused: i == edit.focus,
            hit: None,
        });
    }
    lines.error = edit.error.clone();
}
