//! 配置页改预设（蓝图 `tui.md`「配置页」第 38、39 条；2026-10-08 项目主人定：当场改当场存，不要「以谁为底」，名字只存
//! 一份，新建用 `a`，功能照旧版「启用的功能」那样空格、`Tab` 开关；预设不再管默认人格；2026-10-09 起工具也能一件件
//! 开关，平铺在功能下面）。预设页 `a` 填名字就建好；预设的窗里名字、每一个功能、每一件工具，改一项存一项；`d` 先问再删。

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::Deserialize;
use serde_json::{Value, json};

use super::choose::{LineEdit, PresetView};
use super::presets::{self, Detail};
use super::target::{RemoveTexts, Target, said};
use crate::core::Refusal;
use crate::features;
use crate::settings::popup::{Delete, Popup};
use crate::settings::{Settings, Texts, Tone, Waiting};

/// 改预设的字（`text/zh.json` 的 `settings.more.preset_edit`）。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditTexts {
    /// 新建时编辑窗的标题。
    pub new_name: String,
    /// 删除、恢复出厂前问的。
    pub remove: RemoveTexts,
    /// 删了、这个预设没了。
    pub deleted: String,
    /// 删了自己改的那一份，回到出厂的样子。
    pub restored: String,
}

/// 预设的窗里能改的一行，照画的先后。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    /// 名字。
    Name,
    /// 一个功能、一件工具（平铺的一行；没装的不算，切不了）。
    Row(features::Row),
}

/// 预设的窗里能改的几行。
pub fn fields(detail: &Detail) -> Vec<Field> {
    let rows = features::rows(&detail.features)
        .into_iter()
        .filter(|r| features::selectable(&detail.features, *r))
        .map(Field::Row);
    std::iter::once(Field::Name).chain(rows).collect()
}

/// 几个开关写成要交的值。
fn bools(changes: Vec<(String, bool)>) -> Vec<(String, Value)> {
    changes
        .into_iter()
        .map(|(key, on)| (key, Value::Bool(on)))
        .collect()
}

impl Settings {
    /// 预设页 `a`：开编辑窗写新预设的名字。
    pub(in crate::settings) fn new_preset(&mut self, texts: &Texts) {
        let title = &texts.more.preset_edit.new_name;
        let edit = LineEdit::open("preset.name", title, false, &Value::Null);
        self.popup = Some(Popup::Line(edit.to(Target::NewPreset)));
    }

    /// 预设的窗的按键：上下挪光标；功能那几行空格、`Tab`、`Enter` 切，`Ctrl+A` 全开、全关；名字、默认人格 `Enter`
    /// 改；`d` 删除，`Esc` 关。
    pub(in crate::settings) fn preset_key(
        &mut self,
        mut view: PresetView,
        key: KeyEvent,
        texts: &Texts,
    ) {
        let Some(detail) = view.detail.clone() else {
            if !matches!(key.code, KeyCode::Esc | KeyCode::Char('q')) {
                self.popup = Some(Popup::Preset(view));
            }
            return;
        };
        let lines = fields(&detail);
        let at = lines.get(view.cursor).copied();
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => return,
            KeyCode::Char('a') if ctrl => self.toggle_all(&detail),
            KeyCode::Char('j') | KeyCode::Down => {
                view.cursor = (view.cursor + 1).min(lines.len().saturating_sub(1));
            }
            KeyCode::Char('k') | KeyCode::Up => view.cursor = view.cursor.saturating_sub(1),
            KeyCode::Char('d') => return self.ask_delete_preset(&view.id, texts),
            KeyCode::Char(' ') | KeyCode::Tab | KeyCode::Enter
                if matches!(at, Some(Field::Row(_))) =>
            {
                if let Some(Field::Row(row)) = at {
                    let changes = features::toggle(&detail.features, row);
                    if !changes.is_empty() {
                        self.set_preset(&view.id, bools(changes), false);
                    }
                }
            }
            KeyCode::Enter => return self.edit_preset(view, at, &detail, texts),
            _ => {}
        }
        self.popup = Some(Popup::Preset(view));
    }

    /// `Ctrl+A`：有没开的就全开，都开着才全关（同旧版），一次存。
    fn toggle_all(&mut self, detail: &Detail) {
        let changes = features::toggle_all(&detail.features);
        if !changes.is_empty() {
            self.set_preset(&detail.id, bools(changes), false);
        }
    }

    /// 改名字（编辑窗）。预设不再管默认人格（2026-10-08 项目主人）。
    fn edit_preset(&mut self, view: PresetView, at: Option<Field>, detail: &Detail, texts: &Texts) {
        let words = &texts.more;
        let popup = match at {
            Some(Field::Name) => {
                let current = detail.name.clone().map_or(Value::Null, Value::String);
                let edit = LineEdit::open("preset.name", &words.detail[0], false, &current);
                Popup::Line(edit.to(Target::Preset(Box::new(view))))
            }
            _ => Popup::Preset(view),
        };
        self.popup = Some(popup);
    }

    /// 选择窗、编辑窗选好了：照写到哪办；不是预设的交回 `false`。
    pub(in crate::settings) fn preset_write(
        &mut self,
        target: Target,
        key: &str,
        value: Value,
    ) -> bool {
        let text = value.as_str().unwrap_or_default().trim().to_string();
        match target {
            // 清空了的名字：去掉这一项。
            Target::Preset(view) => {
                let value = if value.is_null() || (value.is_string() && text.is_empty()) {
                    Value::Null
                } else {
                    value
                };
                self.set_preset(&view.id, vec![(key.to_string(), value)], false);
                self.popup = Some(Popup::Preset(*view));
            }
            Target::NewPreset if text.is_empty() => {}
            // 新建：不写编号，核心起（核心 P-3 补）；功能不写就是全开。
            Target::NewPreset => {
                self.set_preset("", vec![("preset.name".into(), json!(text))], true);
            }
            _ => return false,
        }
        true
    }

    /// 写一个预设的几个键（`Null` 是去掉）；`id` 空的是新建。回来了换上详情（[`Settings::preset_saved`]）。
    fn set_preset(&mut self, id: &str, changes: Vec<(String, Value)>, open: bool) {
        let changes: Vec<Value> = changes
            .into_iter()
            .map(|(key, value)| match value {
                Value::Null => json!({"key": key, "unset": true}),
                value => json!({"key": key, "value": value}),
            })
            .collect();
        let mut params = json!({"changes": changes});
        if !id.is_empty() {
            params["preset"] = json!(id);
        }
        self.ask("preset.set", params, Waiting::PresetSaved(open));
    }

    /// `preset.set` 回来了：成了的换上详情（新建的开它的窗）、重读列表；被拒的状态行写原话，详情不变。
    pub(in crate::settings) fn preset_saved(&mut self, open: bool, result: Result<Value, Refusal>) {
        let got = match result {
            Ok(got) => got,
            Err(refusal) => return self.say(said(&refusal), Tone::Bad),
        };
        let detail = presets::detail(&got);
        match &mut self.popup {
            Some(Popup::Preset(view)) if view.id == detail.id => view.detail = Some(detail),
            _ if open => {
                self.popup = Some(Popup::Preset(PresetView {
                    id: detail.id.clone(),
                    detail: Some(detail),
                    error: None,
                    cursor: 0,
                }));
            }
            _ => {}
        }
        self.ask("preset.list", json!({}), Waiting::Presets);
    }

    /// 列表页、预设的窗 `d`：照 `remove` 先问一句。窗开着、读到了的照它；列表页的先读一次。
    pub(in crate::settings) fn ask_delete_preset(&mut self, id: &str, texts: &Texts) {
        let known = match &self.popup {
            Some(Popup::Preset(view)) if view.id == id => {
                view.detail.as_ref().map(|d| d.remove.clone())
            }
            _ => None,
        };
        match known {
            Some(remove) => self.confirm_preset_remove(id, remove.as_deref(), texts),
            None => {
                let waiting = Waiting::PresetRemove(id.to_string());
                self.ask("preset.get", json!({"preset": id}), waiting);
            }
        }
    }

    /// 列表页 `d` 读回来了：照 `remove` 问。
    pub(in crate::settings) fn preset_remove_read(
        &mut self,
        id: &str,
        result: Result<Value, Refusal>,
        texts: &Texts,
    ) {
        match result {
            Ok(got) => {
                let remove = presets::detail(&got).remove;
                self.confirm_preset_remove(id, remove.as_deref(), texts);
            }
            Err(refusal) => self.say(said(&refusal), Tone::Bad),
        }
    }

    fn confirm_preset_remove(&mut self, id: &str, remove: Option<&str>, texts: &Texts) {
        let list = self.more.presets.as_deref().unwrap_or_default();
        let name = list
            .iter()
            .find(|p| p.id == id)
            .map_or(id.to_string(), |p| p.label().to_string());
        let words = &texts.more.preset_edit.remove;
        self.confirm_remove(&name, remove, words, Delete::Preset(id.to_string()));
    }

    /// 问过了、要删：`preset.delete`。
    pub(in crate::settings) fn delete_preset(&mut self, id: &str) {
        self.ask(
            "preset.delete",
            json!({"preset": id}),
            Waiting::PresetDeleted,
        );
    }

    /// 删完了：出厂的回到出厂的样子写「已恢复成出厂的」，自己建的写「已删除」，重读列表；没有能删的照实说。
    pub(in crate::settings) fn preset_deleted(
        &mut self,
        result: Result<Value, Refusal>,
        texts: &Texts,
    ) {
        let words = &texts.more.preset_edit;
        match result {
            Ok(got) => {
                let text = if got["remains"] == true {
                    &words.restored
                } else {
                    &words.deleted
                };
                self.say(text.clone(), Tone::Good);
                self.ask("preset.list", json!({}), Waiting::Presets);
            }
            Err(refusal) if refusal.reason.as_deref() == Some("nothing_to_delete") => {
                self.say(words.remove.nothing.clone(), Tone::Note);
            }
            Err(refusal) => self.say(said(&refusal), Tone::Bad),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{Field, fields};
    use crate::features::Row;
    use crate::settings::pages::presets::detail;

    #[test]
    fn the_window_lists_the_name_then_features_and_their_tools() {
        let got = detail(&json!({"preset": "mine", "features": [
            {"id": "files", "name": "文件读写", "on": true, "installed": true, "tools": [
                {"name": "glob", "label": "找文件", "on": true},
                {"name": "grep", "label": "搜内容", "on": true}]},
            {"id": "goal", "name": "长期目标", "on": true, "installed": false, "tools": []},
            {"id": "commands", "name": "运行命令", "on": false, "installed": true,
             "tools": [{"name": "shell", "label": "执行命令", "on": false}]}]}));
        assert_eq!(
            fields(&got),
            [
                Field::Name,
                Field::Row(Row::Feature(0)),
                Field::Row(Row::Tool(0, 0)),
                Field::Row(Row::Tool(0, 1)),
                Field::Row(Row::Feature(2))
            ],
            "没装的切不了，光标不停；只有一件工具的只有功能那一行"
        );
    }
}
