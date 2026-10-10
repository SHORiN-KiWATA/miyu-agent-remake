//! 画悬浮窗（蓝图「配置页」第 14、15、20 条）：浅一档的底、不画边框；标题、一行一项（名字、值、来源），选中的那项下面
//! 一行说明；按钮、按键提示。编辑窗、选模型的窗、问一句的窗三种。

use ratatui::Frame;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use super::fields::{source, value};
use super::{Look, bar, fit, hint_line};
use crate::caret::Caret;
use crate::settings::forms::{Field, Focus, Form, Target, Val, window_text};
use crate::settings::nav::Use;
use crate::settings::popup::{Choice, Item, Pick, Popup};
use crate::settings::{Hit, Settings, Texts};
use crate::theme;

/// 名字那一栏多宽。
const LABEL: u16 = 14;

/// 画开着的悬浮窗。
pub fn draw(
    frame: &mut Frame,
    area: Rect,
    page: &mut Settings,
    texts: &Texts,
    look: &Look,
    caret: &mut Caret,
) {
    let Some(popup) = page.popup.take() else {
        return;
    };
    let wide = !matches!(
        &popup,
        Popup::Form(Form {
            target: Target::Provider(_),
            ..
        }) | Popup::Confirm(_)
    );
    let width = if wide {
        look.popup_wide
    } else {
        look.popup_width
    }
    .min(area.width.saturating_sub(2));
    let max_height = (area.height * look.popup_height_percent / 100)
        .max(6)
        .min(area.height);
    let mut lines = Lines::default();
    match &popup {
        Popup::Form(form) => form_lines(&mut lines, form, page, texts, width),
        Popup::Pick(pick) => pick_lines(&mut lines, pick, page, texts, width),
        Popup::Confirm(confirm) => {
            lines.title(confirm.title.clone(), String::new());
            for (text, dim) in &confirm.lines {
                lines.plain(Line::styled(
                    text.clone(),
                    if *dim { theme::dim() } else { Style::new() },
                ));
            }
            lines.buttons = confirm
                .buttons
                .iter()
                .map(|(b, danger)| (b.clone(), *danger))
                .collect();
            lines.button = Some(confirm.sel);
            lines.hints = texts.popup_hints[3].clone();
        }
        Popup::Choose(choose) => super::more_popups::choose(&mut lines, choose, texts, width),
        Popup::Line(edit) => super::more_popups::line(&mut lines, edit, texts),
        Popup::Persona(view) => super::more_popups::persona(&mut lines, view, page, texts),
        Popup::Preset(view) => super::more_popups::preset(&mut lines, view, page, texts),
        Popup::Dialogs(view) => super::dialog_popups::dialogs(&mut lines, view, texts, width),
        Popup::Pair(edit) => super::dialog_popups::pair(&mut lines, edit, texts),
    }
    let height = (lines.body.len() as u16 + 8 + u16::from(lines.error.is_some())).min(max_height);
    let rect = Rect::new(
        area.x + (area.width.saturating_sub(width)) / 2,
        area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    );
    paint(frame, rect, &mut lines, page, caret);
    page.popup = Some(popup);
}

/// 窗里的一行行，画之前先排好。
#[derive(Default)]
pub(super) struct Lines {
    pub(super) title: Option<(String, String)>,
    pub(super) body: Vec<Body>,
    pub(super) error: Option<String>,
    pub(super) buttons: Vec<(String, bool)>,
    pub(super) button: Option<usize>,
    pub(super) hints: Vec<[String; 2]>,
    /// 正在改的那一行：第几行、光标在第几列。
    pub(super) caret: Option<(usize, u16)>,
}

/// 一行：字、选中没有、点到它是什么。
pub(super) struct Body {
    pub(super) line: Line<'static>,
    pub(super) focused: bool,
    pub(super) hit: Option<Hit>,
}

impl Lines {
    pub(super) fn title(&mut self, title: String, sub: String) {
        self.title = Some((title, sub));
    }

    pub(super) fn plain(&mut self, line: Line<'static>) {
        self.body.push(Body {
            line,
            focused: false,
            hit: None,
        });
    }
}

fn form_lines(lines: &mut Lines, form: &Form, page: &Settings, texts: &Texts, width: u16) {
    let shown = |id: &str| {
        page.view
            .provider(id)
            .map_or(id.to_string(), |p| p.shown().to_string())
    };
    let (title, sub) = match &form.target {
        Target::Provider(None) => (texts.titles[0].clone(), String::new()),
        Target::Provider(Some(id)) => {
            (texts.titles[1].replace("{name}", &shown(id)), String::new())
        }
        Target::Model(p, None) => (texts.titles[2].replace("{name}", &shown(p)), String::new()),
        Target::Model(p, Some(m)) => (m.clone(), shown(p)),
        Target::Pool(None) => (texts.titles[3].clone(), String::new()),
        Target::Pool(Some(n)) => (texts.titles[4].replace("{name}", n), String::new()),
    };
    lines.title(title, sub);
    let env = form.choice(Field::Auth) == Some("env");
    let value_width = width.saturating_sub(LABEL + 14) as usize;
    for (i, row) in form.rows.iter().enumerate() {
        let focused = form.focus == Focus::Row(i);
        if matches!(row.val, Val::Section) {
            lines.plain(Line::styled(
                format!(" {}", texts.field(row.field.name())),
                theme::md_heading(),
            ));
            continue;
        }
        let editing = focused && form.editing.is_some();
        let (value, caret) = value(row, form, texts, focused, value_width);
        let mut spans = Vec::new();
        if !matches!(row.val, Val::Member { .. }) {
            let name = if row.field == Field::Key && env {
                "env_key"
            } else {
                row.field.name()
            };
            let label = texts.field(name);
            let pad = (LABEL as usize).saturating_sub(label.width() + 2);
            // 选中的那一项名字加粗、用正文色，和没选中的暗字分得开（同一天项目主人：选中的不明显）。
            let style = if focused {
                Style::new().add_modifier(Modifier::BOLD)
            } else {
                theme::hover()
            };
            spans.push(Span::styled(format!("  {label}{}", " ".repeat(pad)), style));
        } else {
            spans.push(Span::raw("  "));
        }
        let start: u16 = spans.iter().map(|s| s.content.width() as u16).sum();
        spans.extend(value);
        let source = source(row, texts);
        let used: usize = spans.iter().map(|s| s.content.width()).sum();
        let right = width.saturating_sub(4) as usize;
        if !source.0.is_empty() && used + source.0.width() + 2 <= right {
            spans.push(Span::raw(" ".repeat(right - used - source.0.width())));
            spans.push(Span::styled(source.0, source.1));
        }
        if editing {
            lines.caret = Some((lines.body.len(), start + caret));
        }
        lines.body.push(Body {
            line: Line::from(spans),
            focused,
            hit: Some(Hit::FormRow(i)),
        });
    }
    lines.error = form.error.clone();
    lines.buttons = texts.buttons.iter().map(|b| (b.clone(), false)).collect();
    lines.button = match form.focus {
        Focus::Button(b) => Some(b),
        Focus::Row(_) => None,
    };
    let which = if form.editing.is_some() { 1 } else { 0 };
    lines.hints = texts.popup_hints[which].clone();
}

fn pick_lines(lines: &mut Lines, pick: &Pick, page: &Settings, texts: &Texts, width: u16) {
    let at = usize::from(pick.usage == Use::Vision);
    let sub = if pick.usage == Use::Vision {
        texts.pick[1].clone()
    } else {
        String::new()
    };
    lines.title(texts.uses[at].clone(), sub);
    let search = if pick.typing || !pick.search.is_empty() {
        Line::raw(format!(" / {}", pick.search))
    } else {
        Line::styled(format!(" {}", texts.pick[4]), theme::dim())
    };
    if pick.typing {
        lines.caret = Some((0, 3 + pick.search.width() as u16));
    }
    lines.plain(search);
    let current = match pick.usage {
        Use::Chat => page.view.chat.clone(),
        Use::Vision => page.view.vision.clone(),
        Use::Embedding => None,
    };
    let right = width.saturating_sub(4) as usize;
    for (i, item) in pick.items(&page.view).into_iter().enumerate() {
        match item {
            Item::Head(name) => {
                let name = name.unwrap_or_else(|| texts.pick[0].clone());
                lines.plain(Line::styled(format!(" {name}"), theme::md_heading()));
            }
            Item::Choice {
                reference,
                label,
                tag,
                off,
            } => {
                let mark = if current.as_deref() == Some(reference.as_str()) {
                    "● "
                } else {
                    "  "
                };
                let tag = match (off, tag) {
                    (Some("off_vision"), _) => texts.pick[2].clone(),
                    (Some(_), _) => texts.pick[3].clone(),
                    (None, Choice::Pool(how, n)) => texts
                        .pool_tag
                        .replace("{how}", texts.option(how.as_deref().unwrap_or_default()))
                        .replace("{n}", &n.to_string()),
                    (None, Choice::Model(window, sees)) => {
                        let mut parts: Vec<String> = window.map(window_text).into_iter().collect();
                        if sees {
                            parts.push(texts.tags[0].clone());
                        }
                        parts.join(" ")
                    }
                };
                let style = if off.is_some() {
                    theme::dim()
                } else {
                    Style::new()
                };
                let text = fit(&label, right.saturating_sub(tag.width() + 6));
                let used = 2 + mark.width() + text.width();
                let pad = right.saturating_sub(used + tag.width());
                let line = Line::from(vec![
                    Span::raw("  "),
                    Span::styled(mark, theme::accent()),
                    Span::styled(text, style),
                    Span::raw(" ".repeat(pad)),
                    Span::styled(tag, theme::dim()),
                ]);
                lines.body.push(Body {
                    line,
                    focused: i == pick.sel,
                    hit: Some(Hit::PickRow(i)),
                });
            }
        }
    }
    lines.hints = texts.popup_hints[2].clone();
}

/// 铺底色、写标题、滚着写一行行、出错的一句、按钮、按键提示。
fn paint(frame: &mut Frame, rect: Rect, lines: &mut Lines, page: &mut Settings, caret: &mut Caret) {
    // 先把格子整个清掉（字色、修饰都不留，不然窗里的字带着后面那页的暗色），再铺底色。
    let buf = frame.buffer_mut();
    for y in rect.y..rect.bottom() {
        for x in rect.x..rect.right() {
            if let Some(cell) = buf.cell_mut(Position::new(x, y)) {
                cell.reset();
            }
        }
    }
    buf.set_style(rect, theme::panel());
    let inner = Rect::new(
        rect.x + 2,
        rect.y + 1,
        rect.width.saturating_sub(4),
        rect.height.saturating_sub(2),
    );
    if let Some((title, sub)) = &lines.title {
        buf.set_string(
            inner.x,
            inner.y,
            fit(title, inner.width as usize),
            theme::accent().add_modifier(Modifier::BOLD),
        );
        let w = sub.width() as u16;
        if !sub.is_empty() && w + title.width() as u16 + 2 < inner.width {
            buf.set_string(inner.right() - w, inner.y, sub, theme::dim());
        }
    }
    let room = inner
        .height
        .saturating_sub(6 + u16::from(lines.error.is_some())) as usize;
    let focus = lines.body.iter().position(|b| b.focused).unwrap_or(0);
    let top = focus
        .saturating_sub(room.saturating_sub(1))
        .min(lines.body.len().saturating_sub(room));
    let top = if lines.caret.is_some_and(|(row, _)| row == 0) {
        0
    } else {
        top
    };
    for (offset, body) in lines.body.iter().skip(top).take(room).enumerate() {
        let y = inner.y + 2 + offset as u16;
        let row = Rect::new(inner.x, y, inner.width, 1);
        if body.focused {
            frame.buffer_mut().set_style(row, theme::row_focus());
            bar(frame, row);
        }
        frame.buffer_mut().set_line(row.x, y, &body.line, row.width);
        if let Some(hit) = body.hit {
            page.hits.push((row, hit));
        }
        if let Some((at, x)) = lines.caret
            && at == top + offset
        {
            caret.put(
                Position::new((row.x + x).min(row.right().saturating_sub(1)), y),
                true,
            );
        }
    }
    let mut y = inner.bottom().saturating_sub(3);
    if let Some(error) = &lines.error {
        frame.buffer_mut().set_string(
            inner.x,
            y - 1,
            fit(error, inner.width as usize),
            theme::error(),
        );
    }
    let mut x = inner.x;
    for (i, (label, danger)) in lines.buttons.iter().enumerate() {
        let text = format!("[ {label} ]");
        let on = lines.button == Some(i);
        let style = match (on, danger) {
            (true, true) => theme::error().add_modifier(Modifier::REVERSED),
            (true, false) => theme::picked_bar(),
            _ => Style::new(),
        };
        let w = text.width() as u16;
        frame.buffer_mut().set_string(x, y, &text, style);
        page.hits.push((Rect::new(x, y, w, 1), Hit::Button(i)));
        x += w + 2;
    }
    // 按钮和按键提示之间空一行（2026-10-07 项目主人）。
    y += 2;
    frame
        .buffer_mut()
        .set_line(inner.x, y, &hint_line(&lines.hints), inner.width);
}
