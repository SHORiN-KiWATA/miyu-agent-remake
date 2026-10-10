//! 选预设那一步的那一块（「第一次打开的引导」第 24–26 条）：一个预设一行，最后一行「自定义」。空格看一个预设的功能、
//! 工具，回车（空格）开自定义，都是浮窗（[`popup`]）。

use ratatui::style::Modifier;
use ratatui::text::Span;
use unicode_width::UnicodeWidthStr;

use super::content::Content;
use crate::features::{self, Feature, Row};
use crate::oobe::Texts;
use crate::oobe::preset::PresetStep;
use crate::theme;

/// 照读回来的拼：一块两行，选着的两行都铺底，右边 `✓`。
pub fn content(step: &PresetStep, texts: &Texts, width: u16) -> Content {
    let words = &texts.preset;
    let mut c = Content::new(width);
    c.heading(&words.head.title, &words.head.sub);
    if step.busy && step.cards.is_empty() {
        c.note(&words.loading, theme::dim());
        return c;
    }
    // 一个预设一行（第 24 条，2026-10-10 项目主人：空格看得到详情，下面那一行功能不用写了）。
    for (i, card) in step.cards.iter().enumerate() {
        let selected = i == step.cursor;
        let mark = selected.then(|| Span::styled("✓", theme::accent()));
        c.row(
            selected,
            vec![Span::styled(card.name.clone(), bold())],
            mark,
        );
    }
    let custom = step.custom();
    c.row(
        custom,
        vec![
            Span::styled(words.custom.clone(), bold()),
            Span::styled(format!("  {}", words.custom_note), theme::dim()),
        ],
        custom.then(|| Span::styled("✓", theme::accent())),
    );
    if let Some(error) = step.error.as_ref().filter(|_| !step.custom_open) {
        c.blank();
        c.note(error, theme::error());
    }
    c
}

/// 开着的浮窗：标题、里面的一块、按键提示；没开的是 `None`。看的浮窗只列功能、工具，照 `view_top` 滚；自定义的浮窗是名字、
/// 功能和工具的开关、「创建 →」（第 24、26 条）。
pub fn popup(
    step: &mut PresetStep,
    texts: &Texts,
    (width, height): (u16, u16),
) -> Option<(String, Content, String)> {
    let words = &texts.preset;
    if step.viewing {
        let card = step.cards.get(step.cursor)?;
        let list = card.features.as_deref().unwrap_or_default();
        let rows = features::rows(list);
        // 放不下的上下各留一行暗色「还有 N 个」，滚到底就停（照搜模型的列表）。
        let height = usize::from(height.max(3));
        let room = if rows.len() > height {
            height - 2
        } else {
            height
        };
        step.view_top = step.view_top.min(rows.len().saturating_sub(room));
        let (above, below) = (&texts.model.more_above, &texts.model.more_below);
        let note = |text: &str, n: usize| {
            ratatui::text::Line::styled(
                format!("  {}", text.replace("{n}", &n.to_string())),
                theme::faint(),
            )
        };
        let mut c = Content::new(width);
        if rows.len() > height {
            c.lines.push(if step.view_top > 0 {
                note(above, step.view_top)
            } else {
                ratatui::text::Line::default()
            });
        }
        let shown = rows.len().min(step.view_top + room);
        for &row in &rows[step.view_top..shown] {
            let mut spans = vec![Span::raw("  ")];
            spans.extend(feature_spans(list, row));
            c.lines.push(ratatui::text::Line::from(spans));
        }
        if shown < rows.len() {
            c.lines.push(note(below, rows.len() - shown));
        }
        return Some((card.name.clone(), c, texts.keys.view.clone()));
    }
    if !step.custom_open {
        return None;
    }
    let mut c = Content::new(width);
    c.cue = texts.keys.enter_edit.clone();
    c.on_panel = true;
    let label_w = words.name.width();
    c.field(
        &words.name,
        label_w,
        &step.name,
        &words.name_hint,
        step.inner == 0 && !step.busy,
    );
    // 名称下面空一行，不和功能挤在一起（第 26 条，2026-10-10 项目主人）。
    c.blank();
    // 功能、工具平铺（「配置页」第 38 条）：功能那一行勾在前、加粗，工具缩进两格。
    for (i, row) in step.rows().into_iter().enumerate() {
        c.row(
            step.inner == i + 1,
            feature_spans(&step.features, row),
            None,
        );
    }
    c.blank();
    c.action(
        step.inner == step.create_row(),
        &format!("{} →", words.create),
    );
    if let Some(error) = &step.error {
        c.blank();
        c.note(error, theme::error());
    }
    let keys = &texts.keys;
    let hint = if step.name.editing() {
        &keys.editing
    } else if step.inner == 0 {
        &keys.custom_name
    } else if step.inner == step.create_row() {
        &keys.create
    } else {
        &keys.custom
    };
    Some((words.custom.clone(), c, hint.clone()))
}

/// 平铺的一行：功能那一行勾在前、加粗，工具缩进两格。
fn feature_spans(list: &[Feature], row: Row) -> Vec<Span<'static>> {
    match row {
        Row::Feature(f) => {
            let feature = &list[f];
            let mark = features::mark(feature);
            vec![
                Span::styled(format!("{} ", mark.text()), check(feature.on)),
                Span::styled(
                    feature.name.clone(),
                    named(feature.on).add_modifier(Modifier::BOLD),
                ),
            ]
        }
        Row::Tool(f, t) => {
            let feature = &list[f];
            let tool = &feature.tools[t];
            let on = features::tool_on(feature, tool);
            let mark = if on { "[*] " } else { "[ ] " };
            vec![
                Span::raw("  "),
                Span::styled(mark, check(on)),
                Span::styled(tool.label.clone(), named(on)),
            ]
        }
    }
}

fn bold() -> ratatui::style::Style {
    ratatui::style::Style::new().add_modifier(Modifier::BOLD)
}

/// 勾：开着强调色，关着暗。
fn check(on: bool) -> ratatui::style::Style {
    if on { theme::accent() } else { theme::dim() }
}

/// 名字：开着照常，关着暗。
fn named(on: bool) -> ratatui::style::Style {
    if on {
        ratatui::style::Style::new()
    } else {
        theme::dim()
    }
}
