//! 全屏的配置页（蓝图 `tui.md`「配置页」）：主菜单、「供应商和模型」三页、悬浮窗。这里只有数据和按键；画在
//! `ui/settings/`，和 App 接在 `app/settings.rs`。和核心说话照 `Command::Ask`：要发的攒在 `asks`，回应照编号认。

pub mod data;
pub mod draft;
mod form_keys;
pub mod forms;
mod input;
pub mod keys;
pub mod merge;
mod mouse;
pub mod nav;
pub mod pages;
pub mod popup;
mod saving;
pub mod texts;

#[cfg(test)]
pub mod test_support;

use std::collections::HashMap;

use serde_json::{Value, json};

use crate::core::Refusal;
pub use data::Data;
use draft::Draft;
use nav::Nav;
use popup::Popup;
pub use texts::Texts;

/// 状态行那一句的颜色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    /// 平常的（黄）。
    Note,
    /// 成了（绿）。
    Good,
    /// 出错（红）。
    Bad,
    /// 正在做（暗）。
    Busy,
}

/// 鼠标点得到的东西（画的时候记下位置，蓝图「配置页」第 6、15 条）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit {
    /// 主菜单的第几项。
    Menu(usize),
    /// 第几个分页。
    Tab(usize),
    /// 一栏的第几行。
    Row(nav::Col, usize),
    /// 编辑窗的第几行。
    FormRow(usize),
    /// 选模型的窗的第几行。
    PickRow(usize),
    /// 悬浮窗的第几个按钮。
    Button(usize),
}

/// 按了一个键以后界面要做的。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// 接着开着。
    Stay,
    /// 回到对话。
    Back,
    /// 退出程序（照 `--page config` 起来的）。
    Quit,
}

/// 等着哪个回应。
#[derive(Debug, Clone)]
enum Waiting {
    /// 读：`model.list`、`config.get`、`secret.list`。
    Load(Load),
    /// 存一个 key：供应商、起的名字。
    Secret(String, String),
    /// 存配置。
    Save,
    /// 测一家：显示名。
    Test(String),
    /// 重取一家的模型列表（`model.list` 带 `refresh`）：编号、显示名。
    Refresh(String, String),
    /// 读配置清单（`config.schema`）：通用、权限、高级这几页照它画。
    Schema,
    /// 读人格（`persona.list`）。
    Personas,
    /// 读一个人格的详情（`persona.get`）：编号。
    PersonaDetail(String),
    /// 读预设（`preset.list`）。
    Presets,
    /// 读装了的吉祥物包（`package.list`，「吉祥物包」第 3 条）。
    Mascots,
    /// 读一个预设的详情（`preset.get`）：编号。
    PresetDetail(String),
    /// 改、建一个预设（`preset.set`，核心 P-3）：存好了要不要开它的窗（新建的）。
    PresetSaved(bool),
    /// 删一个预设的个人那一层（`preset.delete`）。
    PresetDeleted,
    /// 改、建一个人格（`persona.set`，核心 P-3 下）：编号、存好了要不要开它的详情窗（新建的）。
    PersonaSaved(String, bool),
    /// 删一个人格的个人那一层（`persona.delete`）。
    PersonaDeleted,
    /// 传头像（`blob.put`）：人格、换掉的头像的版本（「配置页」第 36 条）。
    AvatarPut(String, Option<String>),
    /// 读一份提示词的原文（`persona.read`），回来了交给编辑器：人格、哪一份。
    PromptRead(String, String),
    /// 读示范对话（`persona.read` 的 `pairs`）：人格。
    ExamplesRead(String),
    /// 存示范对话：人格。
    ExamplesSaved(String),
    /// 列表页 `d`：先读一次详情看能不能删、删了是什么（`remove`）。
    PresetRemove(String),
    /// 同上，人格。
    PersonaRemove(String),
}

#[derive(Debug, Clone, Copy)]
enum Load {
    Models,
    Config,
    Secrets,
}

/// 配置页。
#[derive(Debug, Default)]
pub struct Settings {
    /// 照 `--page config` 起来的：主菜单按 `Esc` 退出程序。
    pub standalone: bool,
    /// 在主菜单上。
    pub on_menu: bool,
    /// 照 `/connect` 直接进的「供应商和模型」：`Esc` 从这一页直接回对话，不经过主菜单。
    direct: bool,
    /// 读来的；还没读到的是 `None`。
    pub data: Option<Data>,
    /// 读来的叠上草稿（界面看的）。
    pub view: Data,
    /// 没存的改动。
    pub draft: Draft,
    /// 页、栏、选中。
    pub nav: Nav,
    /// 开着的悬浮窗。
    pub popup: Option<Popup>,
    /// 通用、权限、高级、人格这几页（`pages/`）。
    pub more: pages::More,
    /// 状态行那一句。
    pub status: Option<(String, Tone)>,
    /// 连着核心。
    pub online: bool,
    /// 鼠标点得到的地方（每帧画的时候重记）。
    pub hits: Vec<(ratatui::layout::Rect, Hit)>,
    /// 上一下点在哪、什么时候：认双击。
    last_click: Option<(ratatui::layout::Position, std::time::Instant)>,
    /// 存成了关掉悬浮窗（`q`、新加的）。
    close_after_save: bool,
    asks: Vec<(u64, &'static str, Value)>,
    waiting: HashMap<u64, Waiting>,
    next: u64,
    incoming: Data,
    loads: usize,
    again: bool,
    secrets_left: usize,
    named: Vec<(String, String)>,
    /// 交给编辑器的那一份提示词（`pages/persona_edit.rs`）：编辑器退出以后照它存。
    prompt: Option<pages::persona_edit::PromptEdit>,
    /// 刚读到、要交给编辑器的：`App` 拿走一次（[`Settings::take_prompt`]）。
    editing: bool,
}

impl Settings {
    /// 打开：停在主菜单，马上去读。
    pub fn open(standalone: bool, online: bool) -> Self {
        let mut page = Self {
            standalone,
            on_menu: true,
            online,
            ..Self::default()
        };
        page.reload();
        page
    }

    /// `/connect`：直接停在「供应商和模型」的供应商页（2026-10-07 项目主人要）。
    pub fn connect(online: bool) -> Self {
        let mut page = Self::open(false, online);
        page.on_menu = false;
        page.direct = true;
        page
    }

    /// 要发给核心的：编号、方法、参数。
    pub fn take_asks(&mut self) -> Vec<(u64, &'static str, Value)> {
        std::mem::take(&mut self.asks)
    }

    fn ask(&mut self, method: &'static str, params: Value, waiting: Waiting) {
        self.next += 1;
        self.waiting.insert(self.next, waiting);
        self.asks.push((self.next, method, params));
    }

    /// 重读三样；正在读的读完再补一次（「配置页」第 25 条）。
    pub fn reload(&mut self) {
        if !self.online {
            return;
        }
        if self.loads > 0 {
            self.again = true;
            return;
        }
        self.loads = 3;
        self.incoming = Data::default();
        self.ask("model.list", json!({}), Waiting::Load(Load::Models));
        self.ask(
            "config.get",
            json!({"all": true}),
            Waiting::Load(Load::Config),
        );
        self.ask("secret.list", json!({}), Waiting::Load(Load::Secrets));
        self.load_more();
    }

    /// 草稿、读来的变了：重拼界面看的那一份，选中的别越界。
    pub fn refresh(&mut self) {
        // 选中的照「是哪一样」留住，不照第几行（重读回来多了、少了几家，选中的不跳）。
        let provider = self.nav.provider(&self.view).map(|p| p.id.clone());
        let model = self.nav.model(&self.view).map(|m| m.name.clone());
        let pool = self.nav.pool(&self.view).map(|p| p.name.clone());
        if let Some(data) = &self.data {
            self.view = merge::merge(data, &self.draft);
        }
        self.nav.clamp(&self.view);
        if let Some(id) = provider {
            self.nav.keep_model(&self.view, &id, model.as_deref());
        }
        if let Some(name) = pool {
            self.nav.keep_pool(&self.view, &name);
        }
    }

    /// 核心交回了一条（`Update::Answer`）；不是这一页发的不管。
    pub fn answer(&mut self, tag: u64, result: Result<Value, Refusal>, texts: &Texts) {
        let Some(waiting) = self.waiting.remove(&tag) else {
            return;
        };
        match waiting {
            Waiting::Load(which) => self.loaded(which, result.ok()),
            Waiting::Secret(provider, name) => self.secret_saved(provider, name, result),
            Waiting::Save => self.saved(result, texts),
            Waiting::Test(name) => self.tested(&name, result, texts),
            Waiting::Refresh(id, name) => self.refreshed(&id, &name, result, texts),
            more @ (Waiting::Schema
            | Waiting::Personas
            | Waiting::PersonaDetail(_)
            | Waiting::Presets
            | Waiting::Mascots
            | Waiting::PresetDetail(_)) => {
                self.more_answer(more, result, &texts.more);
            }
            Waiting::PresetSaved(open) => self.preset_saved(open, result),
            Waiting::PresetDeleted => self.preset_deleted(result, texts),
            Waiting::PersonaSaved(id, open) => self.persona_saved((id, open), result, texts),
            Waiting::PersonaDeleted => self.persona_deleted(result, texts),
            Waiting::AvatarPut(id, version) => self.avatar_put((id, version), result),
            Waiting::PromptRead(id, prompt) => self.prompt_read((id, prompt), result),
            Waiting::ExamplesRead(id) => self.examples_read(id, result),
            Waiting::ExamplesSaved(id) => self.examples_saved(id, result, texts),
            Waiting::PresetRemove(id) => self.preset_remove_read(&id, result, texts),
            Waiting::PersonaRemove(id) => self.persona_remove_read(&id, result, texts),
        }
    }

    fn loaded(&mut self, which: Load, got: Option<Value>) {
        if let Some(got) = got {
            match which {
                Load::Models => self.incoming.read_models(&got),
                Load::Config => self.incoming.read_config(&got),
                Load::Secrets => self.incoming.read_secrets(&got),
            }
        }
        self.loads = self.loads.saturating_sub(1);
        if self.loads > 0 {
            return;
        }
        self.data = Some(std::mem::take(&mut self.incoming));
        self.refresh();
        if std::mem::take(&mut self.again) {
            self.reload();
        }
    }

    /// 配置变了（`config.changed`）：重读，草稿不动。
    pub fn changed(&mut self) {
        self.reload();
    }

    /// 断开了：没回应的都不等了（不重发写请求）。
    pub fn disconnected(&mut self, texts: &Texts) {
        self.online = false;
        self.waiting.clear();
        self.loads = 0;
        self.again = false;
        self.secrets_left = 0;
        self.close_after_save = false;
        self.say(texts.status("offline"), Tone::Bad);
    }

    /// 又连上了：重读。
    pub fn connected(&mut self) {
        self.online = true;
        if matches!(self.status, Some((_, Tone::Bad))) {
            self.status = None;
        }
        self.reload();
    }

    /// 状态行写一句。
    pub fn say(&mut self, text: String, tone: Tone) {
        self.status = Some((text, tone));
    }

    /// 测一家（`r`）：焦点在模型栏的试选中的模型，在供应商栏由核心挑；还没存的新供应商先要存。
    pub fn test(&mut self, texts: &Texts) {
        let Some(p) = self.nav.provider(&self.view) else {
            return;
        };
        let name = p.shown().to_string();
        if self.draft.new_providers.contains(&p.id) {
            self.say(texts.status("save_first"), Tone::Bad);
            return;
        }
        if !self.online {
            self.say(texts.status("offline"), Tone::Bad);
            return;
        }
        let mut params = json!({"provider": p.id});
        if self.nav.focus(&self.view) == nav::Col::Model
            && let Some(m) = self.nav.model(&self.view)
        {
            params["model"] = json!(m.name);
        }
        self.say(texts.status("testing").replace("{name}", &name), Tone::Busy);
        self.ask("provider.test", params, Waiting::Test(name));
    }

    fn tested(&mut self, name: &str, result: Result<Value, Refusal>, texts: &Texts) {
        match result {
            Ok(got) if got["ok"] == true => {
                let n = got["models"].as_array().map_or(0, Vec::len);
                let text = texts
                    .status("tested")
                    .replace("{name}", name)
                    .replace("{n}", &n.to_string());
                self.say(text, Tone::Good);
                self.reload();
            }
            Ok(got) => {
                let why = got["error"]["message"].as_str().unwrap_or_default();
                let text = texts
                    .status("test_failed")
                    .replace("{name}", name)
                    .replace("{why}", why);
                self.say(text, Tone::Bad);
            }
            Err(refusal) => {
                let text = texts
                    .status("test_failed")
                    .replace("{name}", name)
                    .replace("{why}", &refusal.message);
                self.say(text, Tone::Bad);
            }
        }
    }

    fn refreshed(&mut self, id: &str, name: &str, result: Result<Value, Refusal>, texts: &Texts) {
        let got = result.ok();
        let n = got
            .as_ref()
            .and_then(|g| g["providers"].as_array())
            .and_then(|all| all.iter().find(|p| p["id"] == id))
            .and_then(|p| p["models"].as_array())
            .map_or(0, Vec::len);
        let (key, tone) = if n > 0 {
            ("tested", Tone::Good)
        } else {
            ("fetch_failed", Tone::Bad)
        };
        let text = texts
            .status(key)
            .replace("{name}", name)
            .replace("{n}", &n.to_string());
        self.say(text, tone);
        self.reload();
    }

    /// 回主菜单；在主菜单上的回对话（照 `--page config` 起来的退出程序）。
    fn back(&mut self) -> Outcome {
        self.popup = None;
        if self.on_menu || self.direct {
            return if self.standalone {
                Outcome::Quit
            } else {
                Outcome::Back
            };
        }
        self.on_menu = true;
        self.nav.search = None;
        Outcome::Stay
    }

    /// 在打字：编辑窗里正在改一项、在打筛选的字（输入法照它切，`ime.rs`）。
    pub fn typing(&self) -> bool {
        let popup = match &self.popup {
            Some(Popup::Form(form)) => form.editing.is_some(),
            Some(Popup::Pick(pick)) => pick.typing,
            Some(Popup::Line(_) | Popup::Pair(_)) => true,
            _ => false,
        };
        popup || self.nav.search.as_ref().is_some_and(|s| s.typing)
    }

    /// 正在读、还没读到。
    pub fn loading(&self) -> bool {
        self.data.is_none()
    }
}

#[cfg(test)]
mod tests;
