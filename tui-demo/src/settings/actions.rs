//! 配置页的编辑、保存和供应商测试；草稿按个人层快照做冲突检查。

use super::forms::{Entity, Form, Kind};
use super::{Editing, Pending, Settings, data, forms, text};
use crate::input::Editor;
use serde_json::json;
use std::collections::HashMap;

impl Settings {
    /// 打开当前项的草稿，或写单模型默认用途。
    pub fn open(&mut self, texts: &HashMap<String, String>) {
        if !self.connected || !self.loaded {
            self.note = text(texts, "not_connected").into();
            return;
        }
        if self.busy() {
            return;
        }
        if let Some(form) = &self.form {
            if self.selected >= form.fields.len() {
                self.toggle_member();
                return;
            }
            let f = &form.fields[self.selected];
            if matches!(f.kind, Kind::ReadOnly) {
                self.note = text(texts, "read_only").into();
                return;
            }
            let mut editor = Editor::default();
            let secret = matches!(f.kind, Kind::Key);
            // 已有引用可以编辑；多密钥列表只显示掩码，不在单密钥编辑器展开。
            editor.set(
                if secret && !f.value.starts_with("$env:") && !f.value.starts_with("$secret:") {
                    ""
                } else {
                    &f.value
                },
            );
            let (options, checked) = match &f.kind {
                Kind::Choice(v) => (v.clone(), None),
                Kind::Inputs => {
                    let values = serde_json::from_str::<Vec<String>>(&f.value).unwrap_or_default();
                    let options = super::options::get().inputs.clone();
                    let checked = options.iter().map(|v| values.contains(v)).collect();
                    (options, Some(checked))
                }
                _ => (Vec::new(), None),
            };
            let selected = options.iter().position(|o| o == &f.value).unwrap_or(0);
            self.editing = Some(Editing {
                field: self.selected,
                editor,
                options,
                selected,
                checked,
                secret,
            });
            self.note.clear();
            return;
        }
        if self.tab == 0 {
            let Some(p) = self.providers.get(self.provider) else {
                return;
            };
            self.form = if self.column == 0 {
                Some(forms::provider(&self.config, Some(p)))
            } else {
                p.models
                    .get(self.selected)
                    .map(|m| forms::model(&self.config, p, m))
            };
            self.selected = 0;
        } else if self.tab == 3 {
            self.form = self
                .pools
                .get(self.selected)
                .map(|p| forms::pool(&self.config, Some(p)));
            self.selected = 0;
        } else {
            let reference = self
                .models()
                .get(self.selected)
                .map(|(_, m)| m.reference.clone());
            if let Some(reference) = reference {
                let key = if self.tab == 1 {
                    "models.chat"
                } else {
                    "models.vision"
                };
                let change = data::guarded(&self.config, json!({"key":key,"value":reference}));
                self.ask(
                    "config.set",
                    json!({"layer":"personal","changes":[change]}),
                    Pending::Write,
                );
            }
        }
    }

    /// 添加供应商/池：已有标识不编辑，新标识在保存前校验。
    pub fn add(&mut self, texts: &HashMap<String, String>) {
        if !self.connected || !self.loaded {
            self.note = text(texts, "not_connected").into();
            return;
        }
        if self.busy() || self.form.is_some() {
            return;
        }
        self.form = match self.tab {
            0 => Some(forms::provider(&self.config, None)),
            3 => Some(forms::pool(&self.config, None)),
            _ => None,
        };
        self.selected = 0;
        self.note.clear();
    }

    /// 池的选择顺序就是调用先后，不按显示顺序重新排序。
    /// 切换选中模型是否入池，新增成员排在末尾。
    pub(super) fn toggle_member(&mut self) {
        let Some(f) = &self.form else {
            return;
        };
        let field_count = f.fields.len();
        let reference = self
            .models()
            .get(self.selected.saturating_sub(field_count))
            .map(|(_, m)| m.reference.clone());
        if self.selected < field_count {
            return;
        }
        if let (
            Some(reference),
            Some(Form {
                entity: Entity::Pool { members, .. },
                ..
            }),
        ) = (reference, &mut self.form)
        {
            if members.contains(&reference) {
                members.retain(|r| r != &reference);
            } else {
                members.push(reference);
            }
        }
    }

    /// 校验标识后先存密钥，再提交配置引用；密钥值随请求交出即不留在界面。
    pub fn save(&mut self, texts: &HashMap<String, String>) {
        if !self.connected || !self.loaded {
            self.note = text(texts, "not_connected").into();
            return;
        }
        if self.busy() || self.editing.is_some() {
            return;
        }
        let Some(form) = &mut self.form else {
            return;
        };
        if matches!(
            form.entity,
            Entity::Provider { new: true } | Entity::Pool { new: true, .. }
        ) {
            let id = &form.fields[0].value;
            if id.is_empty()
                || id.len() > super::options::get().identifier_max
                || !id.as_bytes()[0].is_ascii_lowercase()
                || !id.bytes().all(|b| {
                    b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'-' | b'_')
                })
            {
                self.note = text(texts, "invalid_id").into();
                return;
            }
            let duplicate = match form.entity {
                Entity::Provider { .. } => self.providers.iter().any(|p| &p.id == id),
                Entity::Pool { .. } => self.pools.iter().any(|p| &p.name == id),
                _ => false,
            };
            if duplicate {
                self.note = text(texts, "duplicate_id").into();
                return;
            }
        }
        if let Some(field) = form.fields.iter_mut().find(|f| {
            matches!(f.kind, Kind::Key)
                && f.changed()
                && !f.value.is_empty()
                && !f.value.starts_with("$env:")
                && !f.value.starts_with("$secret:")
        }) {
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let name = format!("tui-{}-{stamp}", std::process::id());
            let secret = std::mem::replace(&mut field.value, format!("$secret:{name}"));
            self.ask(
                "secret.set",
                json!({"name":name,"value":secret}),
                Pending::Secret,
            );
            self.note = text(texts, "saving").into();
        } else {
            self.write(texts);
        }
    }

    /// 普通配置一次提交；原快照提供个人层逐项 expect。
    pub(super) fn write(&mut self, texts: &HashMap<String, String>) {
        let changes = self.form.as_ref().map_or_else(Vec::new, Form::changes);
        if changes.is_empty() {
            self.note = text(texts, "no_changes").into();
            return;
        }
        self.ask(
            "config.set",
            json!({"layer":"personal","changes":changes}),
            Pending::Write,
        );
        self.note = text(texts, "saving").into();
    }

    /// 测试只面向已保存的供应商，不携带输入中的明文。
    pub fn test(&mut self, texts: &HashMap<String, String>) {
        if !self.connected {
            self.note = text(texts, "not_connected").into();
            return;
        }
        if self.form.is_some() || self.busy() {
            return;
        }
        if let Some(p) = self.providers.get(self.provider) {
            self.ask("provider.test", json!({"provider":p.id}), Pending::Test);
            self.note = text(texts, "loading").into();
        }
    }
}
