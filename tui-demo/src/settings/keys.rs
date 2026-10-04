//! 配置页按键和字段输入；复用字素编辑器，表单不吃对话的输入事件。

use super::{Settings, forms::Entity, text};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::json;
use std::collections::HashMap;

impl Settings {
    /// 返回 true 表示关闭配置页；独立模式由 App 退出。
    pub fn key(&mut self, key: KeyEvent, texts: &HashMap<String, String>) -> bool {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('d') {
            return true;
        }
        if self.editing.is_some() {
            self.edit_key(key);
            return false;
        }
        if self.busy() {
            return false;
        }
        match key.code {
            KeyCode::Esc => {
                if self.form.take().is_some() {
                    self.selected = 0;
                    self.note.clear();
                    self.clamp();
                } else {
                    return true;
                }
            }
            KeyCode::Char('[') | KeyCode::Char(']') if self.form.is_none() => {
                let next = if key.code == KeyCode::Char(']') { 1 } else { 3 };
                self.tab = (self.tab + next) % 4;
                self.selected = 0;
                self.column = 0;
                self.note.clear();
            }
            KeyCode::Up | KeyCode::Char('k') | KeyCode::Down | KeyCode::Char('j') => {
                let count = self.count();
                if count > 0 {
                    let up = matches!(key.code, KeyCode::Up | KeyCode::Char('k'));
                    if self.form.is_none() && self.tab == 0 && self.column == 0 {
                        self.provider = if up {
                            (self.provider + count - 1) % count
                        } else {
                            (self.provider + 1) % count
                        };
                        self.selected = 0;
                    } else {
                        self.selected = if up {
                            (self.selected + count - 1) % count
                        } else {
                            (self.selected + 1) % count
                        };
                    }
                }
            }
            KeyCode::Left | KeyCode::Char('h') | KeyCode::Right | KeyCode::Char('l')
                if self.form.is_none() && self.tab == 0 =>
            {
                self.column = usize::from(matches!(key.code, KeyCode::Right | KeyCode::Char('l')));
                self.selected = 0;
            }
            KeyCode::Enter => self.open(texts),
            KeyCode::Char('a') if self.form.is_none() => self.add(texts),
            KeyCode::Char('p') if self.tab == 0 && self.form.is_none() => self.test(texts),
            KeyCode::Char('s') if self.form.is_some() => self.save(texts),
            KeyCode::Tab | KeyCode::Char(' ')
                if self
                    .form
                    .as_ref()
                    .is_some_and(|f| matches!(f.entity, Entity::Pool { .. })) =>
            {
                self.toggle_member()
            }
            _ => {}
        }
        false
    }

    /// 字段里的粘贴不发送聊天消息、不把多行破坏成表单导航。
    pub fn paste(&mut self, value: &str) {
        if let Some(edit) = &mut self.editing
            && edit.options.is_empty()
        {
            edit.editor.insert(value.trim_matches(['\r', '\n']));
        }
    }

    fn edit_key(&mut self, key: KeyEvent) {
        let Some(edit) = &mut self.editing else {
            return;
        };
        if key.code == KeyCode::Esc {
            self.editing = None;
            return;
        }
        if !edit.options.is_empty() {
            match key.code {
                KeyCode::Down | KeyCode::Char('j') => {
                    edit.selected = (edit.selected + 1) % edit.options.len()
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    edit.selected = (edit.selected + edit.options.len() - 1) % edit.options.len()
                }
                KeyCode::Tab | KeyCode::Char(' ') => {
                    if let Some(checked) = &mut edit.checked {
                        checked[edit.selected] = !checked[edit.selected];
                    }
                }
                KeyCode::Enter => {
                    let value = if let Some(checked) = &edit.checked {
                        let selected: Vec<_> = edit
                            .options
                            .iter()
                            .zip(checked)
                            .filter(|(_, v)| **v)
                            .map(|(s, _)| s)
                            .collect();
                        if selected.is_empty() {
                            String::new()
                        } else {
                            json!(selected).to_string()
                        }
                    } else {
                        edit.options[edit.selected].clone()
                    };
                    if let Some(field) = self
                        .form
                        .as_mut()
                        .and_then(|f| f.fields.get_mut(edit.field))
                    {
                        field.value = value;
                    }
                    self.editing = None;
                }
                _ => {}
            }
            return;
        }
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        let control = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Enter => {
                let value = edit.editor.take();
                if let Some(field) = self
                    .form
                    .as_mut()
                    .and_then(|f| f.fields.get_mut(edit.field))
                {
                    field.value = value;
                }
                self.editing = None;
            }
            KeyCode::Char('a') if control => edit.editor.select_all(),
            KeyCode::Char('u') if control => {
                edit.editor.take();
            }
            KeyCode::Char(c) if !control && !key.modifiers.contains(KeyModifiers::ALT) => {
                edit.editor.insert(&c.to_string())
            }
            KeyCode::Backspace => edit.editor.backspace(),
            KeyCode::Delete => edit.editor.delete(),
            KeyCode::Left => edit.editor.left(shift),
            KeyCode::Right => edit.editor.right(shift),
            KeyCode::Home => edit.editor.move_to(0, shift),
            KeyCode::End => edit.editor.move_to(edit.editor.text().len(), shift),
            _ => {}
        }
    }

    /// 鼠标换分页，编辑中的草稿只能先退出，不静默抹掉。
    pub fn tab(&mut self, tab: usize, texts: &HashMap<String, String>) {
        if self.form.is_some() || self.editing.is_some() || self.busy() {
            self.note = text(texts, "cancel").into();
            return;
        }
        self.tab = tab.min(3);
        self.selected = 0;
        self.column = 0;
        self.note.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{data, forms};

    fn key(c: KeyCode) -> KeyEvent {
        KeyEvent::new(c, KeyModifiers::NONE)
    }

    #[test]
    fn pool_tab_and_space_select_in_order_without_requiring_model_enablement() {
        let mut s = Settings::new(false, true);
        s.loaded = true;
        s.tab = 3;
        s.providers = data::providers(&json!({"providers":[{"id":"p","models":[
            {"model":"a","ref":"p/a"},{"model":"b","ref":"p/b"}]}]}));
        s.form = Some(forms::pool(&json!({}), None));
        s.selected = 3;
        s.key(key(KeyCode::Tab), &HashMap::new());
        s.selected = 2;
        s.key(key(KeyCode::Char(' ')), &HashMap::new());
        let Entity::Pool { members, .. } = &s.form.as_ref().unwrap().entity else {
            panic!()
        };
        assert_eq!(members.as_slice(), ["p/b", "p/a"]);
    }

    #[test]
    fn escape_leaves_the_edit_before_discarding_the_form_before_closing() {
        let mut s = Settings::new(false, true);
        s.loaded = true;
        s.tab = 3;
        s.form = Some(forms::pool(&json!({}), None));
        s.open(&HashMap::new());
        assert!(!s.key(key(KeyCode::Esc), &HashMap::new()));
        assert!(s.form.is_some());
        assert!(!s.key(key(KeyCode::Esc), &HashMap::new()));
        assert!(s.form.is_none());
        assert!(s.key(key(KeyCode::Esc), &HashMap::new()));
    }
    #[test]
    fn clearing_all_input_tags_restores_the_inherited_capabilities() {
        let mut page = Settings::new(false, true);
        page.loaded = true;
        let provider = data::Provider {
            id: "p".into(),
            name: "p".into(),
            models: vec![],
        };
        let model = data::Model {
            id: "a".into(),
            reference: "p/a".into(),
            facts: json!({"inputs":{"value":["text"]}}),
        };
        page.form = Some(forms::model(&json!({}), &provider, &model));
        page.open(&HashMap::new());
        page.key(key(KeyCode::Char(' ')), &HashMap::new());
        page.key(key(KeyCode::Enter), &HashMap::new());
        let changes = page.form.unwrap().changes();
        assert!(
            changes
                .iter()
                .any(|c| c["key"] == "providers.p.models.a.inputs" && c["unset"] == true)
        );
    }
}
