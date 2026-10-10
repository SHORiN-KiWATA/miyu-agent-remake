//! 配置页改人格（蓝图 `tui.md`「配置页」第 35 到 37 条；2026-10-08 项目主人定：只放人要用的四样——名字、人设、示范
//! 对话、角色扮演提示；新建用 `a`、只填名字，编号核心起；不要「以谁为底」）。人设、角色扮演提示回车交给 `$EDITOR`：先
//! `persona.read` 拿原文和版本，编辑器退出读回来，改了的整份交、带版本防覆盖（网页那边先存了的不盖掉）。示范对话是
//! 一对一对的列表（`dialogs.rs`）。`d` 先问再删。

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use serde::Deserialize;
use serde_json::{Value, json};

use super::choose::{LineEdit, PersonaView};
use super::personas;
use super::target::{RemoveTexts, Target, said};
use crate::core::Refusal;
use crate::settings::popup::{Delete, Popup};
use crate::settings::{Settings, Texts, Tone, Waiting};

/// 交给编辑器的两份提示词的名字：人设、角色扮演提示。
pub const PROMPTS: [&str; 2] = ["persona", "reminders"];

/// 改人格的字（`text/zh.json` 的 `settings.more.persona_edit`）。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditTexts {
    /// 新建时编辑窗的标题。
    pub new_name: String,
    /// 删除、恢复出厂前问的。
    pub remove: RemoveTexts,
    /// 删了、这个人格没了（在回收处留 7 天）。
    pub deleted: String,
    /// 删了自己改的那一份，回到出厂的样子。
    pub restored: String,
    /// 头像那一行的名字。
    pub avatar: String,
    /// 有头像时那一行写的。
    pub avatar_set: String,
    /// 填头像路径的编辑窗的标题。
    pub avatar_path: String,
    /// 存的时候别处先改过（`persona_conflict`）。
    pub conflict: String,
    /// 读原文的时候。
    pub reading: String,
}

/// 人格的窗里能改的一行，照画的先后。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    /// 名字。
    Name,
    /// 头像：开编辑窗填图片路径（`avatar.rs`）。
    Avatar,
    /// 一份提示词（[`PROMPTS`] 里的第几个）：交给编辑器。
    Prompt(usize),
    /// 示范对话：开一对一对的列表。
    Examples,
}

/// 人格的窗里的五行：名字、头像、人格提示词、示范对话、人设提醒短语。
pub const FIELDS: [Field; 5] = [
    Field::Name,
    Field::Avatar,
    Field::Prompt(0),
    Field::Examples,
    Field::Prompt(1),
];

/// 交给编辑器的一份提示词：哪个人格、哪一份、读来时的原文和版本、回来以后回到的人格的窗。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptEdit {
    /// 人格的编号。
    pub persona: String,
    /// 哪一份（[`PROMPTS`] 里的名字）。
    pub prompt: String,
    /// 读来时的原文：一个字没改的不存。
    pub original: String,
    /// 读来时的版本，存的时候当 `expect` 带回去；还没有的是 `None`。
    pub version: Option<String>,
    /// 编辑器退出以后回到的人格的窗。
    pub view: PersonaView,
}

impl Settings {
    /// 人格页 `a`：开编辑窗写新人格的名字。
    pub(in crate::settings) fn new_persona(&mut self, texts: &Texts) {
        let title = &texts.more.persona_edit.new_name;
        let edit = LineEdit::open("persona.name", title, false, &Value::Null);
        self.popup = Some(Popup::Line(edit.to(Target::NewPersona)));
    }

    /// 人格的窗的按键：上下挪光标，`Enter` 改这一行，`d` 删除，`Esc` 关。
    pub(in crate::settings) fn persona_key(
        &mut self,
        mut view: PersonaView,
        key: KeyEvent,
        texts: &Texts,
    ) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => return,
            KeyCode::Char('j') | KeyCode::Down => {
                view.cursor = (view.cursor + 1).min(FIELDS.len() - 1);
            }
            KeyCode::Char('k') | KeyCode::Up => view.cursor = view.cursor.saturating_sub(1),
            KeyCode::Char('d') => return self.ask_delete_persona(&view.id, texts),
            KeyCode::Enter if view.detail.is_some() => return self.edit_persona(view, texts),
            _ => {}
        }
        self.popup = Some(Popup::Persona(view));
    }

    /// 改光标上的那一行：名字开编辑窗；人设、角色扮演提示先读原文（回来了交给编辑器）；示范对话先读成一对一对的。
    fn edit_persona(&mut self, view: PersonaView, texts: &Texts) {
        let words = &texts.more;
        let Some(detail) = view.detail.clone() else {
            return;
        };
        let popup = match FIELDS[view.cursor.min(FIELDS.len() - 1)] {
            Field::Avatar => return self.edit_avatar(view, texts),
            Field::Name => {
                let current = detail.name.map_or(Value::Null, Value::String);
                let edit = LineEdit::open("persona.name", &words.detail[0], false, &current);
                Popup::Line(edit.to(Target::Persona(Box::new(view))))
            }
            Field::Prompt(i) => {
                let prompt = PROMPTS[i].to_string();
                let params = json!({"persona": view.id, "prompt": prompt});
                let waiting = Waiting::PromptRead(view.id.clone(), prompt);
                self.ask("persona.read", params, waiting);
                self.say(words.persona_edit.reading.clone(), Tone::Busy);
                Popup::Persona(view)
            }
            Field::Examples => {
                let params = json!({"persona": view.id, "prompt": "examples"});
                let waiting = Waiting::ExamplesRead(view.id.clone());
                self.ask("persona.read", params, waiting);
                self.say(words.persona_edit.reading.clone(), Tone::Busy);
                Popup::Persona(view)
            }
        };
        self.popup = Some(popup);
    }

    /// 编辑窗写好了：照写到哪办；不是人格的交回 `false`。
    pub(in crate::settings) fn persona_write(
        &mut self,
        target: Target,
        key: &str,
        value: Value,
    ) -> bool {
        let text = value.as_str().unwrap_or_default().trim().to_string();
        match target {
            Target::Persona(view) => {
                let value = if text.is_empty() {
                    Value::Null
                } else {
                    json!(text)
                };
                self.set_persona(&view.id, json!({"changes": [change(key, value)]}), false);
                self.popup = Some(Popup::Persona(*view));
            }
            Target::NewPersona if text.is_empty() => {}
            // 新建：不写编号，核心起（核心 P-3 补）；建好开它的窗，光标停在人设，接着填。
            Target::NewPersona => {
                let body = json!({"changes": [change("persona.name", json!(text))]});
                self.set_persona("", body, true);
            }
            _ => return false,
        }
        true
    }

    /// `persona.read` 回来了：记下要交给编辑器的那一份（`App` 拿去开编辑器，[`Settings::take_prompt`]）。
    pub(in crate::settings) fn prompt_read(
        &mut self,
        (persona, prompt): (String, String),
        result: Result<Value, Refusal>,
    ) {
        self.status = None;
        let got = match result {
            Ok(got) => got,
            Err(refusal) => return self.say(said(&refusal), Tone::Bad),
        };
        // 读的时候看着的还是这个人格的详情窗才交给编辑器（读回来以前关了、换了的不开）。
        let view = match &self.popup {
            Some(Popup::Persona(view)) if view.id == persona => view.clone(),
            _ => return,
        };
        self.prompt = Some(PromptEdit {
            persona,
            prompt,
            original: got["text"].as_str().unwrap_or_default().to_string(),
            version: got["version"].as_str().map(str::to_string),
            view,
        });
        self.editing = true;
    }

    /// 要交给编辑器的那一份原文（`App` 写进临时文件、让出终端）。一次只交一次。
    pub fn take_prompt(&mut self) -> Option<String> {
        if !std::mem::take(&mut self.editing) {
            return None;
        }
        self.prompt.as_ref().map(|p| p.original.clone())
    }

    /// 编辑器退出了，读回来的是 `text`（读不了的是 `None`）：一个字没改的不存；改了的整份交、带版本；清空了的存成空的。
    pub fn prompt_edited(&mut self, text: Option<String>) {
        let Some(edit) = self.prompt.take() else {
            return;
        };
        self.popup = Some(Popup::Persona(edit.view));
        // 编辑器常在末尾补一个换行：比改没改时不算末尾的空白。
        let Some(text) = text.filter(|t| t.trim_end() != edit.original.trim_end()) else {
            return;
        };
        // 清空了的就存成空的（核心 P-3 补：`{"text": ""}` 是真的空，回到出厂的走 `d`）。看到什么就是什么。
        let version = edit.version.map_or(Value::Null, Value::String);
        let text = if text.trim().is_empty() {
            String::new()
        } else {
            text
        };
        let body = json!({"text": text, "expect": version});
        let params = json!({"prompts": {edit.prompt: body}});
        self.set_persona(&edit.persona, params, false);
    }

    /// `persona.set`：`body` 里是 `changes`、`prompts`；`id` 空的是新建。回来了换上详情（[`Settings::persona_saved`]）。
    pub(in crate::settings) fn set_persona(&mut self, id: &str, mut body: Value, open: bool) {
        if !id.is_empty() {
            body["persona"] = json!(id);
        }
        let waiting = Waiting::PersonaSaved(id.to_string(), open);
        self.ask("persona.set", body, waiting);
    }

    /// `persona.set` 回来了：成了的换上详情（新建的开它的窗、光标停在人设）、重读列表；别处先改过的不存、重读详情；
    /// 被拒的写原话。
    pub(in crate::settings) fn persona_saved(
        &mut self,
        (id, open): (String, bool),
        result: Result<Value, Refusal>,
        texts: &Texts,
    ) {
        let got = match result {
            Ok(got) => got,
            Err(refusal) if refusal.reason.as_deref() == Some("persona_conflict") => {
                self.say(texts.more.persona_edit.conflict.clone(), Tone::Bad);
                let waiting = Waiting::PersonaDetail(id.clone());
                self.ask("persona.get", json!({"persona": id}), waiting);
                return;
            }
            Err(refusal) => return self.say(said(&refusal), Tone::Bad),
        };
        let detail = personas::detail(&got);
        match &mut self.popup {
            Some(Popup::Persona(view)) if view.id == detail.id => view.detail = Some(detail),
            _ if open => {
                self.popup = Some(Popup::Persona(PersonaView {
                    id: detail.id.clone(),
                    detail: Some(detail),
                    error: None,
                    // 新建的光标停在人格提示词，接着填。
                    cursor: FIELDS
                        .iter()
                        .position(|f| *f == Field::Prompt(0))
                        .unwrap_or(0),
                }));
            }
            _ => {}
        }
        self.ask("persona.list", json!({}), Waiting::Personas);
    }

    /// 列表页、人格的窗 `d`：照 `remove` 先问一句。窗开着、读到了的照它；列表页的先读一次。
    pub(in crate::settings) fn ask_delete_persona(&mut self, id: &str, texts: &Texts) {
        let known = match &self.popup {
            Some(Popup::Persona(view)) if view.id == id => {
                view.detail.as_ref().map(|d| d.remove.clone())
            }
            _ => None,
        };
        match known {
            Some(remove) => self.confirm_persona_remove(id, remove.as_deref(), texts),
            None => {
                let waiting = Waiting::PersonaRemove(id.to_string());
                self.ask("persona.get", json!({"persona": id}), waiting);
            }
        }
    }

    /// 列表页 `d` 读回来了：照 `remove` 问。
    pub(in crate::settings) fn persona_remove_read(
        &mut self,
        id: &str,
        result: Result<Value, Refusal>,
        texts: &Texts,
    ) {
        match result {
            Ok(got) => {
                let remove = personas::detail(&got).remove;
                self.confirm_persona_remove(id, remove.as_deref(), texts);
            }
            Err(refusal) => self.say(said(&refusal), Tone::Bad),
        }
    }

    fn confirm_persona_remove(&mut self, id: &str, remove: Option<&str>, texts: &Texts) {
        let list = self.more.personas.as_deref().unwrap_or_default();
        let name = list
            .iter()
            .find(|p| p.id == id)
            .map_or(id.to_string(), |p| p.label().to_string());
        let words = &texts.more.persona_edit.remove;
        self.confirm_remove(&name, remove, words, Delete::Persona(id.to_string()));
    }

    /// 问过了、要删：`persona.delete`。
    pub(in crate::settings) fn delete_persona(&mut self, id: &str) {
        self.ask(
            "persona.delete",
            json!({"persona": id}),
            Waiting::PersonaDeleted,
        );
    }

    /// 删完了：出厂的回到出厂的样子写「已恢复成出厂的」，自己建的写「已删除」，重读列表；没有能删的照实说。
    pub(in crate::settings) fn persona_deleted(
        &mut self,
        result: Result<Value, Refusal>,
        texts: &Texts,
    ) {
        let words = &texts.more.persona_edit;
        match result {
            Ok(got) => {
                let text = if got["remains"] == true {
                    &words.restored
                } else {
                    &words.deleted
                };
                self.say(text.clone(), Tone::Good);
                self.ask("persona.list", json!({}), Waiting::Personas);
            }
            Err(refusal) if refusal.reason.as_deref() == Some("nothing_to_delete") => {
                self.say(words.remove.nothing.clone(), Tone::Note);
            }
            Err(refusal) => self.say(said(&refusal), Tone::Bad),
        }
    }
}

/// `changes` 的一项：`Null` 是去掉。
fn change(key: &str, value: Value) -> Value {
    match value {
        Value::Null => json!({"key": key, "unset": true}),
        value => json!({"key": key, "value": value}),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::PromptEdit;
    use crate::settings::Settings;
    use crate::settings::pages::choose::PersonaView;

    /// 一个交给了编辑器的人设：原文 `你是她。`，你那一层的版本 `v1`。
    fn editing() -> Settings {
        Settings {
            prompt: Some(PromptEdit {
                persona: "mine".into(),
                prompt: "persona".into(),
                original: "你是她。".into(),
                version: Some("v1".into()),
                view: PersonaView {
                    id: "mine".into(),
                    detail: None,
                    error: None,
                    cursor: 3,
                },
            }),
            ..Settings::default()
        }
    }

    #[test]
    fn an_unchanged_prompt_is_not_saved_and_a_changed_one_carries_its_version() {
        let mut page = editing();
        page.prompt_edited(Some("你是她。\n".into()));
        assert!(page.take_asks().is_empty(), "编辑器补的换行不算改了");
        let mut page = editing();
        page.prompt_edited(None);
        assert!(page.take_asks().is_empty(), "读不回来的不存");
        let mut page = editing();
        page.prompt_edited(Some("你是另一个她。\n".into()));
        let asks = page.take_asks();
        assert_eq!(asks[0].1, "persona.set");
        assert_eq!(
            asks[0].2,
            json!({"persona": "mine", "prompts": {"persona": {"text": "你是另一个她。\n", "expect": "v1"}}})
        );
    }

    #[test]
    fn an_emptied_prompt_is_saved_empty_not_put_back() {
        // 核心 P-3 补：`unset` 是回到出厂的字，清空了要的是空的。
        let mut page = editing();
        page.prompt_edited(Some("\n".into()));
        let asks = page.take_asks();
        assert_eq!(
            asks[0].2["prompts"]["persona"],
            json!({"text": "", "expect": "v1"})
        );
    }
}
