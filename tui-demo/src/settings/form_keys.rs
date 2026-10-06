//! 编辑窗里的按键（蓝图「配置页」第 15 条）：上下挪行、改字、换选项、勾选、到按钮。

use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::forms::{Field, Focus, Form, Val};

/// 编辑窗按了一个键以后要做的。
pub(super) enum FormAction {
    /// 接着开着。
    Stay,
    /// 关掉，不存。
    Close,
    /// 存（`close` 是存成了关窗）。
    Save {
        /// 存成了关窗。
        close: bool,
    },
}

/// 编辑窗的按键：`s` 存了关窗、`Esc` 不存关窗（2026-10-07 项目主人：在悬浮窗里改完直接存，不用回到页面再存）。
pub(super) fn form_key(form: &mut Form, key: KeyEvent) -> FormAction {
    if let Some(editor) = form.editing.as_mut() {
        match key.code {
            KeyCode::Enter => {
                let text = editor.text().trim().to_string();
                if let Focus::Row(i) = form.focus
                    && let Val::Text { text: t, .. } = &mut form.rows[i].val
                {
                    *t = text;
                }
                form.editing = None;
            }
            KeyCode::Esc => form.editing = None,
            KeyCode::Backspace => editor.backspace(),
            KeyCode::Delete => editor.delete(),
            KeyCode::Left => editor.left(false),
            KeyCode::Right => editor.right(false),
            KeyCode::Home => editor.move_to(0, false),
            KeyCode::End => editor.move_to(editor.text().len(), false),
            KeyCode::Char(c) => editor.insert(&c.to_string()),
            _ => {}
        }
        return FormAction::Stay;
    }
    form.error = None;
    match (key.code, form.focus) {
        (KeyCode::Esc, _) => return FormAction::Close,
        // `s` 和按钮「保存」一样：存了关窗（2026-10-07 项目主人）；`q` 同它。
        (KeyCode::Char('s' | 'q'), _) => return FormAction::Save { close: true },
        (KeyCode::Tab, Focus::Row(_)) => form.focus = Focus::Button(0),
        (KeyCode::Tab, Focus::Button(b)) => form.focus = Focus::Button((b + 1) % 2),
        (KeyCode::Char('j') | KeyCode::Down, _) => step(form, 1),
        (KeyCode::Char('k') | KeyCode::Up, _) => step(form, -1),
        (KeyCode::Char('h') | KeyCode::Left, Focus::Button(_)) => form.focus = Focus::Button(0),
        (KeyCode::Char('l') | KeyCode::Right, Focus::Button(_)) => form.focus = Focus::Button(1),
        (KeyCode::Enter, Focus::Button(0)) => return FormAction::Save { close: true },
        (KeyCode::Enter, Focus::Button(_)) => return FormAction::Close,
        (KeyCode::Char('h') | KeyCode::Left, Focus::Row(i)) => turn(form, i, false),
        (KeyCode::Char('l') | KeyCode::Right, Focus::Row(i)) => turn(form, i, true),
        (KeyCode::Char(' '), Focus::Row(i)) => tick(form, i),
        (KeyCode::Enter, Focus::Row(i)) => match &form.rows[i].val {
            Val::Text { text, .. } => {
                // 原来的字整段选中：一敲就换掉，要接着改的按左右键（2026-10-07 实测：窗口 128000 后面接上了 200k）。
                let mut editor = crate::input::Editor::default();
                editor.set(text);
                editor.select_all();
                form.editing = Some(editor);
            }
            Val::Choice { .. } => turn(form, i, true),
            Val::Multi { .. } | Val::Member { .. } => tick(form, i),
            Val::Fixed(_) | Val::Section => {}
        },
        _ => {}
    }
    FormAction::Stay
}

/// 上下挪一行：跳过段名；最后一行再往下到按钮，按钮再往上回最后一行。
fn step(form: &mut Form, dir: isize) {
    let stops: Vec<usize> = (0..form.rows.len())
        .filter(|i| form.rows[*i].stops())
        .collect();
    form.cursor = 0;
    form.focus = match form.focus {
        Focus::Row(i) => {
            let at = stops.iter().position(|s| *s == i).unwrap_or(0);
            match at.checked_add_signed(dir) {
                Some(next) if next < stops.len() => Focus::Row(stops[next]),
                Some(_) => Focus::Button(0),
                None => Focus::Row(i),
            }
        }
        Focus::Button(_) if dir < 0 => stops.last().map_or(Focus::Button(0), |l| Focus::Row(*l)),
        other => other,
    };
}

/// 选项换下一个、上一个；多选挪到下一个、上一个选项。
fn turn(form: &mut Form, i: usize, next: bool) {
    let cursor = &mut form.cursor;
    match &mut form.rows[i].val {
        Val::Choice { options, at } => {
            let n = options.len();
            let now = at.unwrap_or(0);
            *at = Some(if next {
                (now + 1) % n
            } else {
                (now + n - 1) % n
            });
            // 认证换了：原来填的 key、变量名不算数，换成对应的掩码。
            if form.rows[i].field == Field::Auth {
                let env = matches!(&form.rows[i].val, Val::Choice { options, at: Some(a) } if options[*a] == "env");
                if let Some(key) = form.rows.iter_mut().find(|r| r.field == Field::Key) {
                    key.val = Val::Text {
                        text: String::new(),
                        secret: !env,
                    };
                }
            }
        }
        Val::Multi { options, .. } => {
            let n = options.len();
            *cursor = if next {
                (*cursor + 1) % n
            } else {
                (*cursor + n - 1) % n
            };
        }
        _ => {}
    }
}

/// 勾、取消勾：多选勾光标那个（全不勾 = 回到继承的），成员照勾的先后排。
fn tick(form: &mut Form, i: usize) {
    let cursor = form.cursor;
    let row = &mut form.rows[i];
    match &mut row.val {
        Val::Multi { options, on } => {
            let inherited: Vec<bool> = {
                let have = row.inherit.as_deref().unwrap_or_default();
                options
                    .iter()
                    .map(|o| have.split(',').any(|h| h == o))
                    .collect()
            };
            let mut now = on.clone().unwrap_or(inherited);
            now[cursor] = !now[cursor];
            *on = now.iter().any(|b| *b).then_some(now);
        }
        Val::Member { reference, .. } => {
            let reference = reference.clone();
            match form.members.iter().position(|m| *m == reference) {
                Some(at) => {
                    form.members.remove(at);
                }
                None => form.members.push(reference),
            }
        }
        Val::Choice { .. } => turn(form, i, true),
        _ => {}
    }
}
