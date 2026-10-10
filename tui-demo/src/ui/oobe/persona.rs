//! 建人格那一步（「第一次打开的引导」第 21–23 条）：四格和「下一步」；人格提示词、人设提醒短语的大编辑浮窗，示范对话的
//! 列表、两格窗都浮在上面（第 21a 条），示范对话照配置页的字（「配置页」第 41 条）。

use ratatui::text::Span;
use unicode_width::UnicodeWidthStr;

use super::content::{Content, fill, wrap};
use crate::oobe::Texts;
use crate::oobe::persona::{PersonaStep, Slot};
use crate::oobe::sheet::Sheet;
use crate::settings::pages::dialogs::DialogTexts;
use crate::theme;

/// 四格和「下一步」。
pub fn content(step: &PersonaStep, texts: &Texts, width: u16, avatar_rows: u16) -> Content {
    let words = &texts.persona;
    let mut c = Content::new(width);
    c.cue = texts.keys.enter_edit.clone();
    c.heading(&words.head.title, &words.head.sub);
    if step.existing_name.is_some() {
        c.note(&words.existing, theme::good());
        c.blank();
    }
    let label_w = words
        .fields
        .iter()
        .chain([&words.avatar])
        .map(|f| f.width())
        .max()
        .unwrap_or(0);
    let label = |i: usize| words.fields.get(i).cloned().unwrap_or_default();
    let hint = |i: usize| words.hints.get(i).cloned().unwrap_or_default();
    // 浮窗开着时光标在浮窗里，这一屏不亮哪一格。
    let quiet = step.busy || step.writing.is_some() || step.listing || step.editing.is_some();
    let on = |slot| !quiet && step.focus == slot;
    c.field(&label(0), label_w, &step.name, &hint(0), on(Slot::Name));
    // 格子之间空一行（第 21 条，同第 18 条）。
    c.blank();
    // 头像：已有的人格有头像、格子空着的写「已设置」；指着一个文件的，下面画一张小图（第 21 条）。
    let avatar_hint = if step.has_avatar() {
        &words.avatar_set
    } else {
        &words.avatar_hint
    };
    c.field(
        &words.avatar,
        label_w,
        &step.avatar,
        avatar_hint,
        on(Slot::Avatar),
    );
    if let Some(path) = step.avatar_path().filter(|p| p.is_file()) {
        c.picture = Some(super::content::Picture {
            line: c.len(),
            x: u16::try_from(label_w + 4).unwrap_or(u16::MAX),
            path: path.display().to_string(),
            cols: avatar_rows.saturating_mul(3),
            rows: avatar_rows,
        });
        for _ in 0..avatar_rows {
            c.blank();
        }
    }
    c.blank();
    c.chip(
        &label(1),
        label_w,
        &first_line(step.prompt.text()),
        &hint(1),
        on(Slot::Prompt),
    );
    c.blank();
    let count = if step.pairs.is_empty() {
        String::new()
    } else {
        words
            .examples_some
            .replace("{n}", &step.pairs.len().to_string())
    };
    c.chip(&label(2), label_w, &count, &hint(2), on(Slot::Examples));
    c.blank();
    c.chip(
        &label(3),
        label_w,
        &first_line(step.reminders.text()),
        &hint(3),
        on(Slot::Reminders),
    );
    c.blank();
    c.action(on(Slot::Next), &format!("{} →", words.next));
    if step.busy {
        c.blank();
        c.note(&words.saving, theme::dim());
    }
    if let Some(error) = &step.error {
        c.blank();
        c.note(error, theme::error());
    }
    c
}

/// 长文在格子里只写第一行，多行的末尾 `…`。
fn first_line(text: &str) -> String {
    let mut lines = text.trim_end().lines();
    let first = lines.next().unwrap_or_default().to_string();
    if lines.next().is_some() {
        format!("{first} …")
    } else {
        first
    }
}

/// 开着的浮窗：标题、里面的一块、按键提示；都没开的是 `None`。`size` 是窗里写字那一块的宽、高。画大编辑浮窗时记下
/// 折行的宽、滚到哪（上下键照它挪）。
pub fn popup(
    step: &mut PersonaStep,
    texts: &Texts,
    dialogs: &DialogTexts,
    hints: (&str, &str),
    size: (u16, u16),
) -> Option<(String, Content, String)> {
    let words = &texts.persona;
    if let Some((slot, sheet)) = step.writing.as_mut() {
        let at = if *slot == Slot::Prompt { 1 } else { 3 };
        let title = words.fields.get(at).cloned().unwrap_or_default();
        let hint = words.hints.get(at).cloned().unwrap_or_default();
        let body = sheet_body(sheet, &hint, size);
        return Some((title, body, texts.keys.writing.clone()));
    }
    if let Some(draft) = &step.editing {
        let title = if draft.index.is_some() {
            &dialogs.edit
        } else {
            &dialogs.add
        };
        let mut c = Content::new(size.0);
        c.on_panel = true;
        let label_w = dialogs.sides.iter().map(|s| s.width()).max().unwrap_or(0);
        for (i, side) in dialogs.sides.iter().enumerate() {
            if i > 0 {
                c.blank();
            }
            let mut field = crate::oobe::field::Field::default().live();
            field.editor = clone_editor(&draft.sides[i]);
            c.field(side, label_w, &field, "", i == draft.focus);
        }
        if draft.missing {
            c.blank();
            c.note(&dialogs.both, theme::error());
        }
        return Some((title.clone(), c, hints.1.to_string()));
    }
    if !step.listing {
        return None;
    }
    let mut c = Content::new(size.0);
    if step.pairs.is_empty() {
        c.note(&dialogs.empty, theme::dim());
        return Some((dialogs.title.clone(), c, hints.0.to_string()));
    }
    let label_w = dialogs.sides.iter().map(|s| s.width()).max().unwrap_or(0);
    let room = usize::from(size.0).saturating_sub(label_w + 6);
    for (i, pair) in step.pairs.iter().enumerate() {
        let selected = i == step.sel;
        if selected {
            c.focus = Some(c.len() + 1);
        }
        for (side, text) in dialogs.sides.iter().zip([&pair.user, &pair.assistant]) {
            let pad = " ".repeat(label_w.saturating_sub(side.width()));
            let first: String = text
                .lines()
                .next()
                .unwrap_or_default()
                .chars()
                .take(room)
                .collect();
            let spans = vec![
                Span::styled(format!("  {side}{pad}  "), theme::dim()),
                Span::raw(first),
            ];
            c.lines
                .push(fill(spans, size.0, selected.then(theme::row_focus)));
        }
    }
    Some((dialogs.title.clone(), c, hints.0.to_string()))
}

/// 大编辑浮窗里的一块：照宽折行，只露 `size.1` 行，照光标滚；空着的暗色写这一格的说明。
fn sheet_body(sheet: &mut Sheet, hint: &str, (width, height): (u16, u16)) -> Content {
    let width = usize::from(width.max(4)) - 1;
    let height = usize::from(height.max(1));
    sheet.width = width;
    let text = sheet.text().to_string();
    let (rows, (row, col)) = wrap(&text, sheet.field.editor.cursor(), width);
    if row < sheet.top {
        sheet.top = row;
    } else if row >= sheet.top + height {
        sheet.top = row + 1 - height;
    }
    let mut c = Content::new(u16::try_from(width + 1).unwrap_or(u16::MAX));
    if text.is_empty() {
        c.note(hint, theme::faint());
    } else {
        for line in rows.into_iter().skip(sheet.top).take(height) {
            c.lines.push(ratatui::text::Line::raw(line));
        }
    }
    let line = u16::try_from(row - sheet.top).unwrap_or(0);
    c.caret = Some((line, u16::try_from(col).unwrap_or(0)));
    c
}

/// 两格窗的格子借 [`Field`](crate::oobe::field::Field) 画：照原样拷一份字和光标。
fn clone_editor(editor: &crate::input::Editor) -> crate::input::Editor {
    let mut copy = crate::input::Editor::default();
    copy.set(editor.text());
    copy.move_to(editor.cursor(), false);
    copy
}
