//! 示范对话列表（蓝图 `tui.md`「配置页」第 41 条，照旧版的「预设对话」；2026-10-08 项目主人：示范对话要一条 user 对
//! 一条 assistant 地加，不进编辑器，「否则怎么知道格式？」）。一行一轮；`Enter` 改、`a` 加、`d` 删、`J` `K` 挪；改、加
//! 开一个两格的窗。读写都是成对的（核心 P-3 补的 `pairs`），格式归核心；每动一下整份存、带版本防覆盖，存完重读拿新的
//! 版本。

use ratatui::crossterm::event::KeyEvent;
use serde::Deserialize;
use serde_json::{Value, json};

use super::choose::PersonaView;
use super::target::said;
use crate::core::Refusal;
use crate::input::Editor;
use crate::pairs::{self, FormKey, ListKey};
use crate::settings::popup::Popup;
use crate::settings::{Settings, Texts, Tone, Waiting};

/// 示范对话的字（`text/zh.json` 的 `settings.more.dialogs`）。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DialogTexts {
    /// 列表窗的标题。
    pub title: String,
    /// 一轮都没有。
    pub empty: String,
    /// 加一轮的窗的标题。
    pub add: String,
    /// 改这一轮的窗的标题。
    pub edit: String,
    /// 两格的名字：user、assistant。
    pub sides: [String; 2],
    /// 有一格空着按了 `Enter`。
    pub both: String,
}

pub use crate::pairs::Pair;

/// 示范对话列表窗。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialogsView {
    /// 人格的编号。
    pub persona: String,
    /// 一轮轮，照先后。
    pub pairs: Vec<Pair>,
    /// 读来时的版本，存的时候当 `expect`；还没有的是 `None`。
    pub version: Option<String>,
    /// 选中第几轮。
    pub sel: usize,
    /// `Esc` 回到的人格的窗。
    pub back: PersonaView,
}

/// 改、加一轮的两格窗。
#[derive(Debug)]
pub struct PairEdit {
    /// 两格：user、assistant。
    pub sides: [Editor; 2],
    /// 光标在哪一格。
    pub focus: usize,
    /// 改的是第几轮；加的是 `None`（加在末尾）。
    pub index: Option<usize>,
    /// 有一格空着的提示。
    pub error: Option<String>,
    /// 存好、`Esc` 回到的列表。
    pub back: DialogsView,
}

impl PairEdit {
    fn open(pair: &Pair, index: Option<usize>, back: DialogsView) -> Self {
        Self {
            sides: crate::pairs::opened(pair),
            focus: 0,
            index,
            error: None,
            back,
        }
    }
}

/// 读回应里的 `pairs`。
fn read_pairs(got: &Value) -> Vec<Pair> {
    let text = |v: &Value| v.as_str().unwrap_or_default().to_string();
    got["pairs"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|p| Pair {
            user: text(&p["user"]),
            assistant: text(&p["assistant"]),
        })
        .collect()
}

impl Settings {
    /// 示范对话读回来了（`persona.read` 的 `pairs`、`version`）：列表开着的换上（存完重读的），人格的窗开着的开列表。
    pub(in crate::settings) fn examples_read(
        &mut self,
        persona: String,
        result: Result<Value, Refusal>,
    ) {
        self.status = self.status.take().filter(|(_, t)| *t != Tone::Busy);
        let got = match result {
            Ok(got) => got,
            Err(refusal) => return self.say(said(&refusal), Tone::Bad),
        };
        let pairs = read_pairs(&got);
        let version = got["version"].as_str().map(str::to_string);
        match &mut self.popup {
            Some(Popup::Dialogs(view)) if view.persona == persona => {
                view.sel = view.sel.min(pairs.len().saturating_sub(1));
                view.pairs = pairs;
                view.version = version;
            }
            Some(Popup::Persona(back)) if back.id == persona => {
                let back = back.clone();
                self.popup = Some(Popup::Dialogs(DialogsView {
                    persona,
                    pairs,
                    version,
                    sel: 0,
                    back,
                }));
            }
            _ => {}
        }
    }

    /// 列表窗的按键：上下挪，`Enter` 改这一轮，`a` 加一轮，`d` 删，`J` `K` 往下往上挪，`Esc` 回人格的窗。
    pub(in crate::settings) fn dialogs_key(&mut self, mut view: DialogsView, key: KeyEvent) {
        match pairs::list_key(&mut view.pairs, &mut view.sel, key) {
            ListKey::Close => {
                // 回人格的窗，重读一次：示范对话几轮变了。
                let id = view.persona.clone();
                self.ask(
                    "persona.get",
                    json!({"persona": id}),
                    Waiting::PersonaDetail(id),
                );
                self.popup = Some(Popup::Persona(view.back));
                return;
            }
            ListKey::Edit(index) => {
                let pair = index.map(|i| view.pairs[i].clone()).unwrap_or_default();
                self.popup = Some(Popup::Pair(PairEdit::open(&pair, index, view)));
                return;
            }
            ListKey::Changed => self.save_examples(&view),
            ListKey::Stay => {}
        }
        self.popup = Some(Popup::Dialogs(view));
    }

    /// 两格窗的按键：`↑` `↓`（`Tab` 也行）换格；在「你问的」按 `Enter` 跳到「AI回的」，在「AI回的」按 `Enter` 存这一轮
    /// （有一格空着的不存，光标跳过去、写一句）；`Esc` 回列表不存；别的照常打字（2026-10-08 项目主人）。
    pub(in crate::settings) fn pair_key(
        &mut self,
        mut edit: PairEdit,
        key: KeyEvent,
        texts: &Texts,
    ) {
        match pairs::form_key(&mut edit.sides, &mut edit.focus, key) {
            FormKey::Cancel => {
                self.popup = Some(Popup::Dialogs(edit.back));
                return;
            }
            FormKey::Missing => edit.error = Some(texts.more.dialogs.both.clone()),
            FormKey::Done(pair) => {
                let mut view = edit.back;
                pairs::put(&mut view.pairs, &mut view.sel, edit.index, pair);
                self.save_examples(&view);
                self.popup = Some(Popup::Dialogs(view));
                return;
            }
            FormKey::Stay => {}
        }
        self.popup = Some(Popup::Pair(edit));
    }

    /// 两格窗里粘贴：进光标那一格。
    pub(in crate::settings) fn pair_paste(edit: &mut PairEdit, text: &str) {
        edit.sides[edit.focus].insert(text);
    }

    /// 整份存（`persona.set` 的 `prompts.examples`，带版本；一轮都没有的等于删掉）。
    fn save_examples(&mut self, view: &DialogsView) {
        let pairs: Vec<Value> = view
            .pairs
            .iter()
            .map(|p| json!({"user": p.user, "assistant": p.assistant}))
            .collect();
        let version = view.version.clone().map_or(Value::Null, Value::String);
        let params = json!({"persona": view.persona,
            "prompts": {"examples": {"pairs": pairs, "expect": version}}});
        let waiting = Waiting::ExamplesSaved(view.persona.clone());
        self.ask("persona.set", params, waiting);
    }

    /// 存完了：成了的重读一次拿新的版本；别处先改过的、被拒的写一句，也重读（列表回到核心那边的样子）。
    pub(in crate::settings) fn examples_saved(
        &mut self,
        persona: String,
        result: Result<Value, Refusal>,
        texts: &Texts,
    ) {
        match result {
            Ok(_) => {}
            Err(refusal) if refusal.reason.as_deref() == Some("persona_conflict") => {
                self.say(texts.more.persona_edit.conflict.clone(), Tone::Bad);
            }
            Err(refusal) => self.say(said(&refusal), Tone::Bad),
        }
        let params = json!({"persona": persona, "prompt": "examples"});
        self.ask("persona.read", params, Waiting::ExamplesRead(persona));
    }
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use serde_json::json;

    use super::{DialogsView, Pair};
    use crate::config::Config;
    use crate::settings::Settings;
    use crate::settings::pages::choose::PersonaView;
    use crate::settings::popup::Popup;

    fn view(pairs: &[(&str, &str)]) -> DialogsView {
        DialogsView {
            persona: "persona-1".into(),
            pairs: pairs
                .iter()
                .map(|(u, a)| Pair {
                    user: (*u).into(),
                    assistant: (*a).into(),
                })
                .collect(),
            version: Some("v1".into()),
            sel: 0,
            back: PersonaView {
                id: "persona-1".into(),
                detail: None,
                error: None,
                cursor: 3,
            },
        }
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn adding_a_pair_saves_the_whole_list_with_its_version() {
        let texts = Config::builtin().unwrap().text.settings;
        let mut page = Settings::default();
        page.dialogs_key(view(&[("在吗", "在")]), key(KeyCode::Char('a')));
        let Some(Popup::Pair(mut edit)) = page.popup.take() else {
            panic!("开了两格窗");
        };
        edit.sides[0].insert("192 乘 45？");
        // 在「你问的」回车：跳到「AI回的」，不存。
        page.pair_key(edit, key(KeyCode::Enter), &texts);
        let Some(Popup::Pair(edit)) = page.popup.take() else {
            panic!("还在两格窗");
        };
        assert_eq!(edit.focus, 1);
        assert!(edit.error.is_none());
        // 「AI回的」空着回车：不存，写一句。
        page.pair_key(edit, key(KeyCode::Enter), &texts);
        let Some(Popup::Pair(mut edit)) = page.popup.take() else {
            panic!("AI回的空着不存");
        };
        assert!(edit.error.is_some());
        edit.sides[1].insert("8640");
        page.pair_key(edit, key(KeyCode::Enter), &texts);
        let asks = page.take_asks();
        assert_eq!(asks[0].1, "persona.set");
        assert_eq!(
            asks[0].2["prompts"]["examples"],
            json!({"pairs": [{"user": "在吗", "assistant": "在"},
                {"user": "192 乘 45？", "assistant": "8640"}], "expect": "v1"})
        );
        assert!(matches!(page.popup, Some(Popup::Dialogs(ref v)) if v.sel == 1));
    }

    #[test]
    fn moving_and_deleting_save_too_and_escape_goes_back() {
        let mut page = Settings::default();
        page.dialogs_key(view(&[("a", "1"), ("b", "2")]), key(KeyCode::Char('J')));
        let Some(Popup::Dialogs(moved)) = page.popup.take() else {
            panic!("列表还开着");
        };
        assert_eq!(moved.pairs[1].user, "a", "往下挪了一行");
        assert_eq!(moved.sel, 1);
        page.dialogs_key(moved, key(KeyCode::Char('d')));
        let Some(Popup::Dialogs(left)) = page.popup.take() else {
            panic!("列表还开着");
        };
        assert_eq!(left.pairs.len(), 1);
        assert_eq!(page.take_asks().len(), 2, "挪、删各存一次");
        page.dialogs_key(left, key(KeyCode::Esc));
        assert!(matches!(page.popup, Some(Popup::Persona(_))));
    }
}
