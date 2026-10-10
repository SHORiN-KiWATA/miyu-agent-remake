//! 选预设那一步（「第一次打开的引导」第 24–26 条）：一个预设一块，写它开了哪些功能，空格开浮窗看全；最后一块「自定义」，
//! 回车开浮窗：名字、功能和工具的开关（平铺，`crate::features`）。选的写成默认预设，自定义的先建。

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::{Value, json};

use super::Move;
use super::asker::{Asker, Waiting};
use super::field::{Edit, Field};
use super::nav::{self, Nav};
use super::texts::Preset as Texts;
use crate::core::Refusal;
use crate::features::{self, Feature, Row};
use crate::settings::pages::target::said;

/// 一个预设一块。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    /// 编号，不往界面上露。
    pub id: String,
    /// 名字。
    pub name: String,
    /// 装了的功能；`preset.get` 回来以前是 `None`。
    pub features: Option<Vec<Feature>>,
}

/// 这一步。
#[derive(Debug, Default)]
pub struct PresetStep {
    /// 一个预设一块，照 `preset.list` 的先后。
    pub cards: Vec<Card>,
    /// 光标在第几块；等于 `cards.len()` 的是「自定义」。
    pub cursor: usize,
    /// 自定义的名字。
    pub name: Field,
    /// 自定义的功能、工具开关，先照全开。
    pub features: Vec<Feature>,
    /// 自定义展开以后光标在第几行：0 是名字，往下是功能、工具停得上的那几行（[`PresetStep::rows`]）。
    pub inner: usize,
    /// 读回来以前、存的时候。
    pub busy: bool,
    /// 红字。
    pub error: Option<String>,
    /// 选好的名字：好了那一屏写它。
    pub chosen: Option<String>,
    /// 写成默认的那个预设的编号：紧接着的新会话照它开（第 11 条）。
    pub chosen_id: Option<String>,
    default: Option<String>,
    entered: bool,
    /// 光标那一块的功能和工具开着浮窗看（空格，只看不改；第 24 条）。
    pub viewing: bool,
    /// 看的浮窗滚到第几行。
    pub view_top: usize,
    /// 自定义的浮窗开着（第 26 条）。
    pub custom_open: bool,
}

impl PresetStep {
    /// 光标在「自定义」上：它展开着。
    pub fn custom(&self) -> bool {
        self.cursor >= self.cards.len()
    }

    /// 「创建 →」那一行是第几行（名称是第 0 行，接着功能、工具）。
    pub fn create_row(&self) -> usize {
        self.rows().len() + 1
    }

    /// 自定义展开以后光标停得上的功能、工具，照画的先后。
    pub fn rows(&self) -> Vec<Row> {
        features::rows(&self.features)
            .into_iter()
            .filter(|r| features::selectable(&self.features, *r))
            .collect()
    }

    /// 进这一步：第一次进的读列表和默认预设。
    pub fn enter(&mut self, asker: &mut Asker) {
        if self.entered {
            return;
        }
        self.entered = true;
        self.busy = true;
        asker.ask("preset.list", json!({}), Waiting::Presets);
        asker.ask(
            "config.get",
            json!({"keys": ["preset.default"]}),
            Waiting::PresetDefault,
        );
    }

    /// 按了一个键。
    pub fn key(&mut self, key: KeyEvent, asker: &mut Asker, texts: &Texts) -> Move {
        // 看的浮窗：`j` `k` 滚，空格、`Esc` 关（第 24 条）。
        if self.viewing {
            match key.code {
                _ if nav::plain(key) == Some(Nav::Up) => {
                    self.view_top = self.view_top.saturating_sub(1);
                }
                _ if nav::plain(key) == Some(Nav::Down) => self.view_top += 1,
                KeyCode::Char(' ') | KeyCode::Esc => self.viewing = false,
                _ => {}
            }
            return Move::Stay;
        }
        // 自定义的浮窗：编辑名字时 `Esc` 只是不改（第 7 条），不在编辑时关窗（填的、切的留着）。
        if self.custom_open {
            if key.code == KeyCode::Esc && !self.name.editing() {
                self.custom_open = false;
                return Move::Stay;
            }
            if self.busy {
                return Move::Stay;
            }
            return self.custom_key(key, asker, texts);
        }
        if key.code == KeyCode::Esc {
            return Move::Back;
        }
        if self.busy {
            return Move::Stay;
        }
        match key.code {
            _ if nav::plain(key) == Some(Nav::Up) => self.cursor = self.cursor.saturating_sub(1),
            _ if nav::plain(key) == Some(Nav::Down) => {
                self.cursor = (self.cursor + 1).min(self.cards.len());
            }
            KeyCode::Char(' ') if !self.custom() => {
                self.viewing = true;
                self.view_top = 0;
            }
            // 「自定义」回车、空格都开浮窗（第 26 条）。
            KeyCode::Enter | KeyCode::Char(' ') if self.custom() => {
                self.custom_open = true;
                self.inner = 0;
            }
            KeyCode::Enter => {
                if let Some(card) = self.cards.get(self.cursor) {
                    self.chosen = Some(card.name.clone());
                    let id = card.id.clone();
                    self.pick(asker, &id);
                }
            }
            _ => {}
        }
        Move::Stay
    }

    fn custom_key(&mut self, key: KeyEvent, asker: &mut Asker, texts: &Texts) -> Move {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        // 名称那一格 `Enter` 才开始编辑，编辑时 `Enter` 写好、`Esc` 不改；别的时候都认 vim 的键（第 7 条）。
        if self.name.editing() {
            if self.name.edit(key) == Edit::Done {
                self.error = None;
            }
            return Move::Stay;
        }
        let moved = nav::plain(key);
        let rows = self.rows();
        match key.code {
            _ if moved == Some(Nav::Up) => self.inner = self.inner.saturating_sub(1),
            // 最后一行是「创建 →」（[`PresetStep::create_row`]）。
            _ if moved == Some(Nav::Down) => self.inner = (self.inner + 1).min(self.create_row()),
            KeyCode::Char(' ') | KeyCode::Tab | KeyCode::Enter
                if self.inner > 0 && self.inner < self.create_row() =>
            {
                if let Some(row) = rows.get(self.inner - 1) {
                    let changes = features::toggle(&self.features, *row);
                    features::apply(&mut self.features, &changes);
                }
            }
            KeyCode::Char('a') if ctrl => {
                let changes = features::toggle_all(&self.features);
                features::apply(&mut self.features, &changes);
            }
            KeyCode::Enter if self.inner == 0 => self.name.begin(),
            KeyCode::Enter if self.inner == self.create_row() => {
                let name = self.name.trimmed();
                if name.is_empty() {
                    self.error = Some(texts.name_needed.clone());
                    self.inner = 0;
                    return Move::Stay;
                }
                let mut changes = vec![json!({"key": "preset.name", "value": name})];
                changes.extend(
                    features::offs(&self.features)
                        .into_iter()
                        .map(|(key, on)| json!({"key": key, "value": on})),
                );
                self.busy = true;
                self.chosen = Some(name);
                asker.ask(
                    "preset.set",
                    json!({"changes": changes}),
                    Waiting::PresetSave,
                );
            }
            _ => {}
        }
        Move::Stay
    }

    /// 粘贴进自定义的名字。
    pub fn paste(&mut self, text: &str) {
        if self.custom_open && self.inner == 0 {
            if !self.name.editing() {
                self.name.begin();
            }
            self.name.paste(text);
        }
    }

    /// 写成默认预设。
    fn pick(&mut self, asker: &mut Asker, id: &str) {
        self.busy = true;
        self.error = None;
        let params =
            json!({"layer": "personal", "changes": [{"key": "preset.default", "value": id}]});
        asker.ask("config.set", params, Waiting::PresetPick(id.to_string()));
    }

    /// 等的回来了。
    pub fn answer(
        &mut self,
        waiting: &Waiting,
        result: Result<Value, Refusal>,
        asker: &mut Asker,
        _texts: &Texts,
    ) -> Move {
        match waiting {
            Waiting::Presets => {
                let got = result.unwrap_or_default();
                self.cards = got["presets"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|p| p["problem"].is_null())
                    .filter_map(|p| {
                        let id = p["preset"].as_str()?.to_string();
                        let name = p["name"].as_str().unwrap_or(&id).to_string();
                        Some(Card {
                            id,
                            name,
                            features: None,
                        })
                    })
                    .collect();
                for card in &self.cards {
                    asker.ask(
                        "preset.get",
                        json!({"preset": card.id}),
                        Waiting::PresetGet(card.id.clone()),
                    );
                }
                self.busy = false;
                self.aim();
            }
            Waiting::PresetDefault => {
                let got = result.unwrap_or_default();
                self.default = got["items"]["preset.default"]["value"]
                    .as_str()
                    .map(str::to_string);
                self.aim();
            }
            Waiting::PresetGet(id) => {
                let got = result.unwrap_or_default();
                let list: Vec<Feature> = features::read(&got)
                    .into_iter()
                    .filter(|f| f.installed)
                    .collect();
                // 自定义的先照全开：功能、工具都开着。
                if self.features.is_empty() {
                    self.features = list
                        .iter()
                        .cloned()
                        .map(|mut f| {
                            f.on = true;
                            f.tools.iter_mut().for_each(|t| t.on = true);
                            f
                        })
                        .collect();
                }
                if let Some(card) = self.cards.iter_mut().find(|c| c.id == *id) {
                    card.features = Some(list);
                }
            }
            Waiting::PresetSave => match result {
                Ok(got) => {
                    let id = got["preset"].as_str().unwrap_or_default().to_string();
                    self.pick(asker, &id);
                }
                Err(refusal) => self.refused(&refusal),
            },
            Waiting::PresetPick(id) => match result {
                Ok(_) => {
                    self.busy = false;
                    self.chosen_id = Some(id.clone());
                    return Move::Next;
                }
                Err(refusal) => self.refused(&refusal),
            },
            _ => {}
        }
        Move::Stay
    }

    /// 列表和默认都知道了：光标停在默认预设那一块。
    fn aim(&mut self) {
        if let Some(at) = self
            .default
            .as_ref()
            .and_then(|d| self.cards.iter().position(|c| c.id == *d))
        {
            self.cursor = at;
        }
    }

    fn refused(&mut self, refusal: &Refusal) {
        self.busy = false;
        self.error = Some(said(refusal));
    }
}
