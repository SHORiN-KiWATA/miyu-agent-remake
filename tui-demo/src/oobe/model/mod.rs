//! 接模型那一步（「第一次打开的引导」第 15–20 条）：已经有聊天模型的问用它还是换；选一家；填密钥或者自定义；试；
//! 选模型；存。画在 `ui/oobe/model.rs`。

mod form;
pub mod more;
pub mod plan;
mod replies;
pub mod rows;

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use serde_json::{Value, json};

use super::asker::{Asker, Waiting};
use super::field::Edit;
use super::nav::{self, Nav};
use super::texts::Model as Texts;
use super::{Mood, Move};
pub use form::{Form, Models, Slot};
use plan::{KeyRef, Target};
use rows::Row;

/// 走到哪一屏。
#[derive(Debug, Default)]
pub enum Phase {
    /// 读回来以前。
    #[default]
    Loading,
    /// 已经有聊天模型：选着「用它」（`true`）还是「换一个」。
    Ready(bool),
    /// 选一家。
    List,
    /// 填密钥、自定义。
    Form(Form),
    /// 在试。
    Testing(Form),
    /// 选模型。
    Models(Models),
    /// 在存。
    Saving(Models),
}

/// 这一步。
#[derive(Debug, Default)]
pub struct ModelStep {
    /// 走到哪一屏。
    pub phase: Phase,
    /// 选一家那一屏的一行行。
    pub rows: Vec<Row>,
    /// 选一家那一屏光标在第几行。
    pub cursor: usize,
    /// 红字。
    pub error: Option<String>,
    /// 黄字（列不出模型）。
    pub warn: Option<String>,
    /// 现在的聊天模型（`models.chat` 的显示名）。
    pub chat: Option<String>,
    /// 这一次选好的模型：好了那一屏写它。
    pub chosen: Option<String>,
    /// 试通了以后吉祥物的反应，引导拿走。
    pub mood: Option<Mood>,
    /// 「更多供应商…」浮窗开着（第 16a 条）。
    pub more: Option<more::More>,
    /// 拿全目录时 `provider.catalog` 的 `limit`（`oobe.json` 的 `catalog_limit`）。
    pub catalog_limit: u64,
    catalog: Option<Value>,
    detect: Option<Value>,
    list: Option<Value>,
    pools: Option<bool>,
    configured: Vec<String>,
}

impl ModelStep {
    /// 新的一步：拿全目录时 `limit` 写 `catalog_limit`。
    pub fn new(catalog_limit: u64) -> Self {
        Self {
            catalog_limit,
            ..Self::default()
        }
    }

    /// 进这一步：第一次进的要三份回应和配置里有没有池；回来的照原样。
    pub fn enter(&mut self, asker: &mut Asker) {
        if !matches!(self.phase, Phase::Loading) || asker.pending(&Waiting::Models) {
            return;
        }
        asker.ask("model.list", json!({}), Waiting::Models);
        asker.ask("provider.detect", json!({}), Waiting::Detect);
        asker.ask(
            "provider.catalog",
            json!({"featured": true}),
            Waiting::Catalog,
        );
        asker.ask("config.get", json!({}), Waiting::Pools);
    }

    /// 按了一个键。
    pub fn key(&mut self, key: KeyEvent, asker: &mut Asker, texts: &Texts) -> Move {
        // 「更多供应商…」浮窗开着：键归它（第 16a 条）。
        if let Some(mut more) = self.more.take() {
            match more.key(key) {
                more::MoreKey::Stay => self.more = Some(more),
                more::MoreKey::Close => {}
                more::MoreKey::Pick(provider) => {
                    let form = Form {
                        provider: Some(provider),
                        ..Form::default()
                    };
                    self.fill_in(form, asker, texts);
                }
            }
            return Move::Stay;
        }
        match std::mem::take(&mut self.phase) {
            Phase::Loading => {
                self.phase = Phase::Loading;
                if key.code == KeyCode::Esc {
                    return Move::Back;
                }
            }
            Phase::Ready(keep) => match key.code {
                _ if matches!(nav::plain(key), Some(Nav::Up | Nav::Down)) => {
                    self.phase = Phase::Ready(!keep);
                }
                KeyCode::Enter if keep => {
                    self.chosen = self.chat.clone();
                    self.phase = Phase::Ready(keep);
                    return Move::Next;
                }
                KeyCode::Enter => self.phase = Phase::List,
                KeyCode::Esc => {
                    self.phase = Phase::Ready(keep);
                    return Move::Back;
                }
                _ => self.phase = Phase::Ready(keep),
            },
            Phase::List => return self.list_key(key, asker, texts),
            Phase::Form(form) => return self.form_key(form, key, asker, texts),
            Phase::Testing(form) => self.phase = Phase::Testing(form),
            Phase::Models(models) => return self.models_key(models, key, asker),
            Phase::Saving(models) => self.phase = Phase::Saving(models),
        }
        Move::Stay
    }

    /// 粘贴进光标那一格。
    pub fn paste(&mut self, text: &str) {
        if let Some(more) = self.more.as_mut() {
            more.paste(text);
            return;
        }
        match &mut self.phase {
            Phase::Form(form) => {
                if let Some(slot) = form.slot()
                    && let Some(field) = form.field(slot)
                {
                    if !field.editing() {
                        field.begin();
                    }
                    field.paste(text);
                }
            }
            Phase::Models(models) => {
                if !models.filter.editing() {
                    models.filter.begin();
                }
                models.filter.paste(text);
            }
            _ => {}
        }
    }

    fn list_key(&mut self, key: KeyEvent, asker: &mut Asker, texts: &Texts) -> Move {
        self.phase = Phase::List;
        let step = |from: usize, down: bool, rows: &[Row]| {
            let mut at = from;
            loop {
                at = if down { at + 1 } else { at.checked_sub(1)? };
                if rows.get(at)?.selectable() {
                    return Some(at);
                }
            }
        };
        match key.code {
            _ if nav::plain(key) == Some(Nav::Up) => {
                self.cursor = step(self.cursor, false, &self.rows).unwrap_or(self.cursor);
            }
            _ if nav::plain(key) == Some(Nav::Down) => {
                self.cursor = step(self.cursor, true, &self.rows).unwrap_or(self.cursor);
            }
            KeyCode::Esc if self.chat.is_some() => self.phase = Phase::Ready(true),
            KeyCode::Esc => return Move::Back,
            KeyCode::Enter => {
                self.error = None;
                self.warn = None;
                let form = match self.rows.get(self.cursor) {
                    Some(Row::Provider(p)) if p.supported => Form {
                        provider: Some(p.clone()),
                        ..Form::default()
                    },
                    Some(Row::Custom) => Form::default(),
                    Some(Row::More) => {
                        self.more = Some(more::More::open());
                        let params = json!({"limit": self.catalog_limit});
                        asker.ask("provider.catalog", params, Waiting::AllProviders);
                        return Move::Stay;
                    }
                    _ => return Move::Stay,
                };
                self.fill_in(form, asker, texts);
            }
            _ => {}
        }
        Move::Stay
    }

    fn form_key(
        &mut self,
        mut form: Form,
        key: KeyEvent,
        asker: &mut Asker,
        texts: &Texts,
    ) -> Move {
        let slots = form.slots();
        let last = slots.len().saturating_sub(1);
        let slot = form.slot();
        // 正在编辑一格：`Enter` 写好、`Esc` 不改，别的照常打字（第 7 条）。
        if let Some(field) = slot.and_then(|s| form.field(s)).filter(|f| f.editing()) {
            if field.edit(key) == Edit::Done {
                self.error = None;
                if slot == Some(Slot::Url) && plan::url(form.url.text()).is_none() {
                    self.error = Some(texts.url_bad.clone());
                }
            }
            self.phase = Phase::Form(form);
            return Move::Stay;
        }
        let kinds = plan::DRIVERS.len();
        match key.code {
            KeyCode::Esc => {
                self.error = None;
                self.warn = None;
                self.phase = Phase::List;
                return Move::Stay;
            }
            _ if nav::plain(key) == Some(Nav::Up) => form.focus = form.focus.saturating_sub(1),
            _ if nav::plain(key) == Some(Nav::Down) => form.focus = (form.focus + 1).min(last),
            _ if slot == Some(Slot::Driver) && nav::plain(key) == Some(Nav::Left) => {
                form.driver = (form.driver + kinds - 1) % kinds;
            }
            _ if slot == Some(Slot::Driver) && nav::plain(key) == Some(Nav::Right) => {
                form.driver = (form.driver + 1) % kinds;
            }
            KeyCode::Enter => match slot {
                Some(Slot::Driver) => form.driver = (form.driver + 1) % kinds,
                Some(Slot::Test) => {
                    self.error = None;
                    if let Some(missing) = self.missing(&form, texts) {
                        self.error = Some(missing.0);
                        form.focus = missing.1;
                    } else {
                        self.test(form, asker, texts);
                        return Move::Stay;
                    }
                }
                Some(s) => {
                    if let Some(field) = form.field(s) {
                        field.begin();
                    }
                }
                None => {}
            },
            _ => {}
        }
        self.phase = Phase::Form(form);
        Move::Stay
    }

    /// 选好一家：不用填的直接试，要填的进填写那一屏。
    fn fill_in(&mut self, form: Form, asker: &mut Asker, texts: &Texts) {
        if form.slots().is_empty() {
            self.test(form, asker, texts);
        } else {
            self.phase = Phase::Form(form);
        }
    }

    /// 还没填好的：说什么、光标跳到第几格。
    fn missing(&self, form: &Form, texts: &Texts) -> Option<(String, usize)> {
        let slots = form.slots();
        let at = |slot| slots.iter().position(|s| *s == slot).unwrap_or(0);
        if form.provider.is_none() && plan::url(form.url.text()).is_none() {
            return Some((texts.url_bad.clone(), at(Slot::Url)));
        }
        if form.provider.is_some() && slots.contains(&Slot::Key) && form.key.trimmed().is_empty() {
            return Some((texts.key_needed.clone(), at(Slot::Key)));
        }
        None
    }

    /// 发 `provider.test`，吉祥物抬头等着。
    fn test(&mut self, form: Form, asker: &mut Asker, _texts: &Texts) {
        let Some(target) = form.target() else {
            self.phase = Phase::Form(form);
            return;
        };
        let key = form.key.trimmed();
        let model = form.model.trimmed();
        let params = plan::test_params(&target, Some(&key), Some(&model));
        asker.ask("provider.test", params, Waiting::Test);
        self.mood = Some(Mood::Wait);
        self.phase = Phase::Testing(form);
    }

    fn models_key(&mut self, mut models: Models, key: KeyEvent, asker: &mut Asker) -> Move {
        let last = models.matches().len().saturating_sub(1);
        // 搜的时候打字是筛，挪光标另认 Ctrl+J/K/N/P；`Enter` 写好、`Esc` 不搜了（第 7 条）。
        if models.filter.editing() {
            match nav::search(key) {
                Some(Nav::Up) => models.cursor = models.cursor.saturating_sub(1),
                Some(Nav::Down) => models.cursor = (models.cursor + 1).min(last),
                _ => {
                    if models.filter.edit(key) == Edit::Stay {
                        models.cursor = 0;
                    }
                }
            }
            self.phase = Phase::Models(models);
            return Move::Stay;
        }
        match key.code {
            KeyCode::Esc => {
                self.phase = Phase::List;
                return Move::Stay;
            }
            KeyCode::Char('/') => models.filter.begin(),
            _ if nav::plain(key) == Some(Nav::Up) => {
                models.cursor = models.cursor.saturating_sub(1)
            }
            _ if nav::plain(key) == Some(Nav::Down) => {
                models.cursor = (models.cursor + 1).min(last)
            }
            KeyCode::Enter => {
                if let Some(name) = models.matches().get(models.cursor).map(|n| (*n).clone()) {
                    self.save(models, &name, asker);
                    return Move::Stay;
                }
            }
            _ => {}
        }
        self.phase = Phase::Models(models);
        Move::Stay
    }

    /// 存：贴了 key 的先 `secret.set`，回来再写配置。
    fn save(&mut self, models: Models, name: &str, asker: &mut Asker) {
        self.error = None;
        let Some(target) = models.form.target() else {
            return;
        };
        let (id, _) = plan::config_id(&target, &self.configured);
        let key = models.form.key.trimmed();
        self.chosen = Some(name.to_string());
        if needs_secret(&target, &key) {
            asker.ask(
                "secret.set",
                json!({"name": id, "value": key}),
                Waiting::Secret,
            );
        } else {
            self.write(&target, &key, name, asker);
        }
        self.phase = Phase::Saving(models);
    }

    /// 写这一家和 `models.chat`。
    fn write(&mut self, target: &Target, key: &str, model: &str, asker: &mut Asker) {
        let (id, catalog) = plan::config_id(target, &self.configured);
        let key_ref = match target {
            Target::Listed(p) if p.env.is_some() => KeyRef::Env(p.env.clone().unwrap_or_default()),
            _ if needs_secret(target, key) => KeyRef::Secret,
            Target::Listed(p) if p.base_url.is_some() => KeyRef::Local,
            _ => KeyRef::None,
        };
        let pools = self.pools.unwrap_or(false);
        let params = plan::save_params(target, (&id, catalog.as_deref()), &key_ref, model, pools);
        asker.ask("config.set", params, Waiting::SaveModel);
    }
}

/// 贴了 key、又不是找到的环境变量：要存进密钥库。
fn needs_secret(target: &Target, key: &str) -> bool {
    !key.is_empty()
        && !matches!(target, Target::Listed(p) if p.env.is_some() || p.configured.is_some())
}

#[cfg(test)]
mod tests;
