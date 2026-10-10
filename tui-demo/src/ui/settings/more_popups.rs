//! 新几页的悬浮窗（蓝图「配置页」第 33、34、36、38 条）：下拉的选择窗（现在的值前面一个点，选不了的暗着、后面黄字写
//! 原因）、填一行字的编辑窗、人格的窗、预设的窗。和别的悬浮窗一个底、一个排法（`popup.rs` 的 `Lines`）。

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use super::fit;
use super::popup::{Body, Lines};
use crate::features;
use crate::settings::pages::choose::{Choose, LineEdit, PersonaView, PresetView};
use crate::settings::pages::preset_edit::{Field, fields};
use crate::settings::{Hit, Settings, Texts};
use crate::theme;

/// 选择窗。
pub(super) fn choose(lines: &mut Lines, choose: &Choose, texts: &Texts, width: u16) {
    lines.title(choose.title.clone(), String::new());
    let room = usize::from(width.saturating_sub(6));
    for (i, c) in choose.choices.iter().enumerate() {
        let style = if c.off.is_some() {
            theme::dim()
        } else {
            Style::new()
        };
        let mut spans = vec![Span::raw("  "), Span::styled(fit(&c.label, room), style)];
        let used = c.label.width() + 2;
        if let Some(off) = &c.off {
            spans.push(Span::styled(
                fit(&format!("  {off}"), room.saturating_sub(used)),
                theme::warn(),
            ));
        } else if let Some(note) = &c.note {
            spans.push(Span::styled(
                fit(&format!("  {note}"), room.saturating_sub(used)),
                theme::dim(),
            ));
        }
        lines.body.push(Body {
            line: Line::from(spans),
            focused: i == choose.sel,
            hit: Some(Hit::PickRow(i)),
        });
    }
    lines.hints = texts.more.hints[2].clone();
}

/// 编辑窗：一行输入框，原来的字下划线，光标跟着。
pub(super) fn line(lines: &mut Lines, edit: &LineEdit, texts: &Texts) {
    lines.title(edit.title.clone(), String::new());
    let typed = edit.editor.text();
    let before = &typed[..edit.editor.cursor().min(typed.len())];
    let style = Style::new().add_modifier(Modifier::UNDERLINED);
    lines.caret = Some((
        lines.body.len(),
        2 + u16::try_from(before.width()).unwrap_or(0),
    ));
    lines.body.push(Body {
        line: Line::from(vec![
            Span::raw("  "),
            Span::styled(typed.to_string(), style),
        ]),
        focused: true,
        hit: None,
    });
    lines.error = edit.error.clone();
    lines.hints = texts.more.hints[3].clone();
}

/// 人格的窗（蓝图「配置页」第 36 条）：名字、人设、示范对话、角色扮演提示四行，光标那一行铺底色；没读回来写「正在读…」。
pub(super) fn persona(lines: &mut Lines, view: &PersonaView, page: &Settings, texts: &Texts) {
    let words = &texts.more;
    let label = page
        .more
        .personas
        .iter()
        .flatten()
        .find(|p| p.id == view.id)
        .map_or(view.id.clone(), |p| p.label().to_string());
    lines.title(label, String::new());
    lines.hints = words.hints[6].clone();
    let Some(detail) = &view.detail else {
        let text = view.error.clone().unwrap_or_else(|| words.loading.clone());
        lines.plain(Line::styled(format!("  {text}"), theme::dim()));
        return;
    };
    // 没有的空着（「配置页」第 36 条）。
    let none = String::new;
    let written = |yes: bool| if yes { words.detail[5].clone() } else { none() };
    let examples = if detail.examples == 0 {
        none()
    } else {
        words.detail[4].replace("{n}", &detail.examples.to_string())
    };
    let avatar = &words.persona_edit;
    let avatar_value = if detail.avatar.is_some() {
        avatar.avatar_set.clone()
    } else {
        none()
    };
    // 照 `FIELDS` 的先后：名字、头像、人格提示词、示范对话、人设提醒短语。
    let rows = [
        (&words.detail[0], detail.name.clone().unwrap_or_else(none)),
        (&avatar.avatar, avatar_value),
        (&words.detail[1], written(detail.prompt)),
        (&words.detail[2], examples),
        (&words.detail[3], written(detail.reminders)),
    ];
    let label_w = rows.iter().map(|(n, _)| n.width()).max().unwrap_or(0);
    for (i, (name, value)) in rows.into_iter().enumerate() {
        let pad = " ".repeat(label_w.saturating_sub(name.width()));
        lines.body.push(Body {
            line: Line::from(vec![
                Span::styled(format!("  {name}{pad}  "), theme::dim()),
                Span::raw(value),
            ]),
            focused: i == view.cursor,
            hit: None,
        });
    }
}

/// 预设的窗（蓝图「配置页」第 38 条）：名字；「功能 N/M」一段照旧版「启用的功能」一行一个 `[*] 名字   说明`，
/// 没装的暗着、右边黄字「没安装」；关掉的单件工具一行，写显示名。光标那一行铺底色。
pub(super) fn preset(lines: &mut Lines, view: &PresetView, page: &Settings, texts: &Texts) {
    let words = &texts.more;
    let label = page
        .more
        .presets
        .iter()
        .flatten()
        .find(|p| p.id == view.id)
        .map_or(view.id.clone(), |p| p.label().to_string());
    lines.title(label, String::new());
    lines.hints = words.hints[7].clone();
    let Some(detail) = &view.detail else {
        let text = view.error.clone().unwrap_or_else(|| words.loading.clone());
        lines.plain(Line::styled(format!("  {text}"), theme::dim()));
        return;
    };
    let at = fields(detail).get(view.cursor).copied();
    let none = String::new;
    lines.body.push(Body {
        line: Line::from(vec![
            Span::styled(format!("  {}  ", words.detail[0]), theme::dim()),
            Span::raw(detail.name.clone().unwrap_or_else(none)),
        ]),
        focused: at == Some(Field::Name),
        hit: None,
    });
    // 功能：照旧版一行一个开关，空格、Tab 切（2026-10-08 项目主人）；工具平铺在功能下面，一件一行（2026-10-09）。
    let installed: Vec<_> = detail.features.iter().filter(|f| f.installed).collect();
    let on = installed.iter().filter(|f| f.on).count();
    let title = words.preset_detail[0]
        .replace("{on}", &on.to_string())
        .replace("{all}", &installed.len().to_string());
    lines.plain(Line::default());
    lines.plain(Line::styled(
        format!("  {title}"),
        theme::dim().add_modifier(Modifier::BOLD),
    ));
    for row in features::rows(&detail.features) {
        lines.body.push(Body {
            line: feature_line(&detail.features, row, &words.preset_detail[1]),
            focused: at == Some(Field::Row(row)),
            hit: None,
        });
    }
}

/// 平铺的一行（「配置页」第 38 条）：功能那一行勾在前、名字加粗；工具缩进两格；关着的、功能关着时的工具暗着；没装的
/// 暗着、名字后面黄字「没安装」。
fn feature_line(list: &[features::Feature], row: features::Row, missing: &str) -> Line<'static> {
    let checked = |on: bool| {
        if on {
            ("[*]", theme::accent(), Style::new())
        } else {
            ("[ ]", theme::dim(), theme::dim())
        }
    };
    match row {
        features::Row::Feature(i) => {
            let feature = &list[i];
            if !feature.installed {
                return Line::from(vec![
                    Span::styled(format!("      {}", feature.name), theme::dim()),
                    Span::raw("  "),
                    Span::styled(missing.to_string(), theme::warn()),
                ]);
            }
            let mark = features::mark(feature);
            let (_, mark_style, name_style) = checked(feature.on);
            Line::from(vec![
                Span::raw("  "),
                Span::styled(mark.text(), mark_style),
                Span::raw(" "),
                Span::styled(
                    feature.name.clone(),
                    name_style.add_modifier(Modifier::BOLD),
                ),
            ])
        }
        features::Row::Tool(i, j) => {
            let feature = &list[i];
            let tool = &feature.tools[j];
            let (mark, mark_style, name_style) = checked(features::tool_on(feature, tool));
            Line::from(vec![
                Span::raw("    "),
                Span::styled(mark, mark_style),
                Span::raw(" "),
                Span::styled(tool.label.clone(), name_style),
            ])
        }
    }
}
