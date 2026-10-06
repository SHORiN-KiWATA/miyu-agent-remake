//! 配置页的按键（蓝图「配置页」第 5、6、11、15、16 条）：主菜单、三页、筛、悬浮窗各管各的。

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::form_keys::{FormAction, form_key};
use super::forms::{self, Form, Target};
use super::nav::{Col, Page, Search};
use super::popup::{Confirm, Delete, Pick, Popup};
use super::{Outcome, Settings, Texts, Tone};

/// 删一样东西时，一个引用算不算在用它。
type Uses = Box<dyn Fn(&str) -> bool>;

impl Settings {
    /// 按了一个键。
    pub fn key(&mut self, key: KeyEvent, texts: &Texts) -> Outcome {
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            return Outcome::Stay;
        }
        if self.popup.is_some() {
            return self.popup_key(key, texts);
        }
        if self.nav.search.as_ref().is_some_and(|s| s.typing) {
            self.search_key(key);
            return Outcome::Stay;
        }
        if self.on_menu {
            return match key.code {
                KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => {
                    self.on_menu = false;
                    self.status = None;
                    Outcome::Stay
                }
                KeyCode::Esc | KeyCode::Char('q') => self.back(),
                _ => Outcome::Stay,
            };
        }
        if self.loading() {
            return match key.code {
                KeyCode::Esc => self.back(),
                _ => Outcome::Stay,
            };
        }
        self.status = self.status.take().filter(|(_, t)| *t == Tone::Busy);
        self.page_key(key, texts)
    }

    /// 粘贴：正在改的那一项、正在打的筛选字收下（贴 key 是最常见的用法），换行去掉。
    pub fn paste(&mut self, text: &str) {
        let clean: String = text.chars().filter(|c| !c.is_control()).collect();
        let clean = clean.trim();
        match &mut self.popup {
            Some(Popup::Form(form)) => {
                if let Some(editor) = form.editing.as_mut() {
                    editor.insert(clean);
                }
            }
            Some(Popup::Pick(pick)) if pick.typing => {
                pick.search.push_str(clean);
                pick.refilter(&self.view);
            }
            _ => {
                if let Some(search) = self.nav.search.as_mut().filter(|s| s.typing) {
                    search.text.push_str(clean);
                }
            }
        }
    }

    fn page_key(&mut self, key: KeyEvent, texts: &Texts) -> Outcome {
        let view = &self.view;
        match key.code {
            KeyCode::Char('h') | KeyCode::Left => self.nav.sideways(false, view),
            KeyCode::Char('l') | KeyCode::Right => self.nav.sideways(true, view),
            KeyCode::Char('j') | KeyCode::Down => self.nav.vertical(1, view),
            KeyCode::Char('k') | KeyCode::Up => self.nav.vertical(-1, view),
            KeyCode::PageDown => self.nav.vertical(10, view),
            KeyCode::PageUp => self.nav.vertical(-10, view),
            // 开着中文输入法时收到的是全角的。
            KeyCode::Char(',' | '，' | '<') => self.nav.page_step(false),
            KeyCode::Char('.' | '。' | '>') => self.nav.page_step(true),
            KeyCode::Enter => self.enter(),
            KeyCode::Char('a') => self.add(texts),
            KeyCode::Char('n') if self.nav.page == Page::Providers => {
                if let Some(p) = self.nav.provider(&self.view) {
                    let id = p.id.clone();
                    self.open_form(|v, d, dr| forms::model(v, d, dr, &id, None));
                }
            }
            KeyCode::Char('d') => self.delete(texts),
            KeyCode::Char('r') if self.nav.page == Page::Providers => self.test(texts),
            KeyCode::Char('/') => {
                let col = self.nav.focus(view);
                if matches!(col, Col::Provider | Col::Model | Col::Pool) {
                    self.nav.search = Some(Search {
                        col: Some(col),
                        text: String::new(),
                        typing: true,
                    });
                }
            }
            // `q` 和 `Esc` 一样返回（2026-10-07 项目主人）。
            KeyCode::Esc | KeyCode::Char('q') => return self.leave(),
            _ => {}
        }
        Outcome::Stay
    }

    /// 在筛：打字、删字，`Enter` 收起（筛的留着），`Esc` 不筛了。
    fn search_key(&mut self, key: KeyEvent) {
        let Some(search) = self.nav.search.as_mut() else {
            return;
        };
        match key.code {
            KeyCode::Enter => {
                search.typing = false;
                if search.text.is_empty() {
                    self.nav.search = None;
                }
            }
            KeyCode::Esc => self.nav.search = None,
            KeyCode::Backspace => {
                search.text.pop();
            }
            KeyCode::Char(c) => search.text.push(c),
            KeyCode::Down => self.nav.vertical(1, &self.view),
            KeyCode::Up => self.nav.vertical(-1, &self.view),
            _ => {}
        }
        if let Some(col) = self.nav.search.as_ref().and_then(|s| s.col)
            && matches!(key.code, KeyCode::Char(_) | KeyCode::Backspace)
        {
            self.nav.pick(col, 0);
        }
        self.nav.clamp(&self.view);
    }

    /// `Esc`：在筛的先不筛了，不然回主菜单（改动都是悬浮窗里当场存的，没有要问的）。
    fn leave(&mut self) -> Outcome {
        if self.nav.search.take().is_some() {
            return Outcome::Stay;
        }
        self.back()
    }

    /// `Enter`：照焦点在哪一栏开窗。
    fn enter(&mut self) {
        let col = self.nav.focus(&self.view);
        match col {
            Col::Provider => {
                if let Some(id) = self.nav.provider(&self.view).map(|p| p.id.clone()) {
                    self.open_form(|v, d, dr| forms::provider(v, d, dr, Some(&id)));
                }
            }
            Col::Org => self.nav.sideways(true, &self.view),
            Col::Model => {
                let p = self.nav.provider(&self.view).map(|p| p.id.clone());
                let m = self.nav.model(&self.view).map(|m| m.name.clone());
                if let (Some(p), Some(m)) = (p, m) {
                    self.open_form(|v, d, dr| forms::model(v, d, dr, &p, Some(&m)));
                }
            }
            Col::Use => {
                self.popup = Some(Popup::Pick(Pick::open(self.nav.usage(), &self.view)));
            }
            Col::Pool | Col::Member => {
                if let Some(name) = self.nav.pool(&self.view).map(|p| p.name.clone()) {
                    self.popup = Some(Popup::Form(forms::pool(&self.view, Some(&name))));
                }
            }
        }
    }

    fn open_form(&mut self, build: impl FnOnce(&super::Data, &super::Data, &super::Draft) -> Form) {
        if let Some(data) = &self.data {
            self.popup = Some(Popup::Form(build(&self.view, data, &self.draft)));
        }
    }

    /// `a`：供应商页加供应商，模型池页建池。
    fn add(&mut self, texts: &Texts) {
        match self.nav.page {
            Page::Providers => self.open_form(|v, d, dr| forms::provider(v, d, dr, None)),
            Page::Pools => self.popup = Some(Popup::Form(forms::pool(&self.view, None))),
            Page::Defaults => self.say(texts.status("no_new"), Tone::Note),
        }
    }

    /// `d`：先问，列出谁在用它（「配置页」第 13 条）。
    fn delete(&mut self, texts: &Texts) {
        let col = self.nav.focus(&self.view);
        let Some(data) = &self.data else {
            return;
        };
        let (title, ask, prefix, uses): (String, Delete, String, Uses) = match col {
            Col::Provider => {
                let Some(p) = self.nav.provider(&self.view) else {
                    return;
                };
                let id = p.id.clone();
                let head = format!("{id}/");
                (
                    texts.confirm[0].replace("{name}", p.shown()),
                    Delete::Provider(id.clone()),
                    super::keys::provider(&id),
                    Box::new(move |r: &str| r.starts_with(&head)),
                )
            }
            Col::Model => {
                let (Some(p), Some(m)) =
                    (self.nav.provider(&self.view), self.nav.model(&self.view))
                else {
                    return;
                };
                if !m.custom() {
                    self.say(texts.status("not_custom"), Tone::Bad);
                    return;
                }
                let reference = m.reference.clone();
                (
                    texts.confirm[1].replace("{name}", &m.name),
                    Delete::Model(p.id.clone(), m.name.clone()),
                    m.table.clone(),
                    Box::new(move |r: &str| r == reference),
                )
            }
            Col::Pool => {
                let Some(pool) = self.nav.pool(&self.view) else {
                    return;
                };
                let reference = format!("@{}", pool.name);
                (
                    texts.confirm[2].replace("{name}", &pool.name),
                    Delete::Pool(pool.name.clone()),
                    super::keys::pool(&pool.name),
                    Box::new(move |r: &str| r == reference),
                )
            }
            Col::Member => return self.say(texts.status("members_hint"), Tone::Note),
            _ => return self.say(texts.status("nothing_here"), Tone::Note),
        };
        if data.in_system(&prefix) {
            self.say(texts.status("in_system"), Tone::Bad);
            return;
        }
        // 只说谁在用它，一行（2026-10-07 项目主人：原来那样太啰嗦）；没人用的什么都不写。
        let mut users = Vec::new();
        for (i, used) in [&self.view.chat, &self.view.vision].iter().enumerate() {
            if used.as_deref().is_some_and(&uses) {
                users.push(texts.uses[i].clone());
            }
        }
        for pool in &self.view.pools {
            if pool.members.iter().any(|m| uses(m)) {
                users.push(texts.confirm[4].replace("{name}", &pool.name));
            }
        }
        let mut lines = Vec::new();
        if !users.is_empty() {
            let list = users.join(&texts.confirm[6]);
            lines.push((texts.confirm[3].replace("{list}", &list), true));
        }
        self.popup = Some(Popup::Confirm(Confirm {
            title,
            lines,
            buttons: vec![
                (texts.confirm[5].clone(), true),
                (texts.buttons[1].clone(), false),
            ],
            sel: 1,
            ask,
        }));
    }

    fn popup_key(&mut self, key: KeyEvent, texts: &Texts) -> Outcome {
        match self.popup.take() {
            Some(Popup::Form(mut form)) => match form_key(&mut form, key) {
                FormAction::Stay => self.popup = Some(Popup::Form(form)),
                FormAction::Close => {}
                FormAction::Save { close } => self.save_form(form, close, texts),
            },
            Some(Popup::Pick(pick)) => self.pick_key(pick, key, texts),
            Some(Popup::Confirm(confirm)) => return self.confirm_key(confirm, key, texts),
            None => {}
        }
        Outcome::Stay
    }

    /// 编辑窗里存（`s`、`q`、「保存」）：照窗里的值记进这一次的改动，马上存；新加的存成了关窗（再按一次 `s` 就成了
    /// 重名）。窗在回应到以前开着，没存成的原因写在窗里。
    fn save_form(&mut self, form: Form, close: bool, texts: &Texts) {
        let Some(data) = &self.data else {
            return;
        };
        let new = matches!(
            &form.target,
            Target::Provider(None) | Target::Model(_, None) | Target::Pool(None)
        );
        match forms::apply(&form, &self.view, data, &mut self.draft) {
            Ok(id) => {
                self.refresh();
                match &form.target {
                    Target::Provider(None) => self.nav.show_model(&self.view, &id, None),
                    Target::Model(p, None) => {
                        let model = id.strip_prefix(&format!("{p}/")).map(str::to_string);
                        self.nav.show_model(&self.view, p, model.as_deref());
                    }
                    Target::Pool(None) => self.nav.show_pool(&self.view, &id),
                    _ => {}
                }
                self.popup = Some(Popup::Form(form));
                self.save(close || new, texts);
            }
            Err((error, name)) => {
                let mut form = form;
                form.error = Some(texts.error(error).replace("{name}", &name));
                self.popup = Some(Popup::Form(form));
            }
        }
    }

    fn pick_key(&mut self, mut pick: Pick, key: KeyEvent, texts: &Texts) {
        let keep = if pick.typing {
            match key.code {
                KeyCode::Enter => pick.typing = false,
                KeyCode::Esc => {
                    pick.typing = false;
                    pick.search.clear();
                    pick.refilter(&self.view);
                }
                KeyCode::Backspace => {
                    pick.search.pop();
                    pick.refilter(&self.view);
                }
                KeyCode::Char(c) => {
                    pick.search.push(c);
                    pick.refilter(&self.view);
                }
                KeyCode::Down => pick.step(1, &self.view),
                KeyCode::Up => pick.step(-1, &self.view),
                _ => {}
            }
            true
        } else {
            match key.code {
                KeyCode::Char('j') | KeyCode::Down => pick.step(1, &self.view),
                KeyCode::Char('k') | KeyCode::Up => pick.step(-1, &self.view),
                KeyCode::Char('/') => pick.typing = true,
                KeyCode::Enter => {
                    if let (Some(reference), Some(data)) = (pick.chosen(&self.view), &self.data) {
                        let key = match pick.usage {
                            super::nav::Use::Chat => "models.chat",
                            super::nav::Use::Vision => "models.vision",
                        };
                        self.draft.set(key, serde_json::json!(reference), data);
                        self.refresh();
                        self.save(true, texts);
                        return;
                    }
                }
                KeyCode::Esc if !pick.search.is_empty() => {
                    pick.search.clear();
                    pick.refilter(&self.view);
                }
                KeyCode::Esc => return,
                _ => {}
            }
            true
        };
        if keep {
            self.popup = Some(Popup::Pick(pick));
        }
    }

    fn confirm_key(&mut self, mut confirm: Confirm, key: KeyEvent, texts: &Texts) -> Outcome {
        let n = confirm.buttons.len();
        match key.code {
            KeyCode::Char('h' | 'k') | KeyCode::Left | KeyCode::Up => {
                confirm.sel = (confirm.sel + n - 1) % n
            }
            KeyCode::Char('l' | 'j') | KeyCode::Right | KeyCode::Down | KeyCode::Tab => {
                confirm.sel = (confirm.sel + 1) % n;
            }
            KeyCode::Enter => return self.decide(&confirm.ask, confirm.sel, texts),
            KeyCode::Esc => return Outcome::Stay,
            _ => {}
        }
        self.popup = Some(Popup::Confirm(confirm));
        Outcome::Stay
    }

    /// 问的那一句选了第几个按钮：删除的当场存。
    fn decide(&mut self, ask: &Delete, button: usize, texts: &Texts) -> Outcome {
        let Some(data) = &self.data else {
            return Outcome::Stay;
        };
        if button != 0 {
            return Outcome::Stay;
        }
        let prefix = match ask {
            Delete::Provider(id) => super::keys::provider(id),
            Delete::Model(p, m) => super::keys::model_table(p, m),
            Delete::Pool(name) => super::keys::pool(name),
        };
        self.draft.unset_all(&prefix, data);
        self.refresh();
        self.save(true, texts);
        Outcome::Stay
    }
}
