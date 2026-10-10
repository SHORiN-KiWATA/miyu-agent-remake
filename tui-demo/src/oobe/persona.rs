//! 建人格那一步（「第一次打开的引导」第 21–23 条）：名字、头像、人格提示词、示范对话、人设提醒短语五格，名字必填；「下一步」
//! 一次 `persona.set` 建好、写成默认人格。已经有默认人格的照它填好，改了的带版本存。人格提示词、人设提醒短语在大编辑浮窗
//! 里写（`sheet.rs`），示范对话的列表、两格窗也浮在这一步上面（第 21a 条，2026-10-09 项目主人）。

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use serde_json::json;

use super::asker::{Asker, Waiting};
use super::field::{Edit, Field};
use super::nav::{self, Nav};
use super::sheet::{Sheet, SheetKey};
use super::texts::Persona as Texts;
use super::{Effect, Mood, Move};
use crate::input::Editor;
use crate::pairs::{self, FormKey, ListKey, Pair};

/// 光标停的几格，照画的先后。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Slot {
    /// 名字。
    #[default]
    Name,
    /// 头像：图片的路径（2026-10-10 项目主人）。
    Avatar,
    /// 人格提示词（2026-10-09 项目主人从「人设」改名）。
    Prompt,
    /// 示范对话。
    Examples,
    /// 人设提醒短语（同一天从「角色扮演提示」改名）。
    Reminders,
    /// 「下一步」。
    Next,
}

/// 照画的先后。
pub const SLOTS: [Slot; 6] = [
    Slot::Name,
    Slot::Avatar,
    Slot::Prompt,
    Slot::Examples,
    Slot::Reminders,
    Slot::Next,
];

/// 三份提示词的名字（`persona.read` 的 `prompt`）。
const PROMPTS: [&str; 3] = ["persona", "reminders", "examples"];

/// 改、加一轮示范对话的两格。
#[derive(Debug)]
pub struct PairDraft {
    /// 你问的、AI回的。
    pub sides: [Editor; 2],
    /// 光标在哪一格。
    pub focus: usize,
    /// 改第几轮；加的是 `None`。
    pub index: Option<usize>,
    /// 有一格空着按了回车。
    pub missing: bool,
}

/// 已经有的默认人格：读来的样子和版本（存的时候只交改了的、带版本）。
#[derive(Debug, Clone, Default)]
struct Existing {
    id: String,
    name: String,
    texts: [Option<String>; 2],
    pairs: Option<Vec<Pair>>,
    versions: [Option<String>; 3],
    /// 头像的版本（`persona.get` 的 `avatar`）；没有头像的是 `None`。
    avatar: Option<String>,
    read: usize,
}

/// 这一步。
#[derive(Debug)]
pub struct PersonaStep {
    /// 名字。
    pub name: Field,
    /// 头像的路径。
    pub avatar: Field,
    /// 传上去了的头像（`blob.put` 的 `blob`）：路径改了就作废。
    avatar_blob: Option<String>,
    /// 人格提示词。
    pub prompt: Field,
    /// 人设提醒短语。
    pub reminders: Field,
    /// 示范对话。
    pub pairs: Vec<Pair>,
    /// 示范对话列表选着第几轮。
    pub sel: usize,
    /// 示范对话列表开着。
    pub listing: bool,
    /// 两格窗开着。
    pub editing: Option<PairDraft>,
    /// 大编辑浮窗开着：写的是哪一格。
    pub writing: Option<(Slot, Sheet)>,
    /// 光标在哪一格。
    pub focus: Slot,
    /// 读回来以前、存的时候。
    pub busy: bool,
    /// 红字。
    pub error: Option<String>,
    /// 已经有默认人格：标题下面写「已建好，可以接着改」。
    pub existing_name: Option<String>,
    /// 建好了的名字：好了那一屏写它。
    pub chosen: Option<String>,
    /// 写成默认的那个人格的编号：紧接着的新会话照它开（第 11 条）。
    pub chosen_id: Option<String>,
    /// 刚建好：名字写到吉祥物脚下，引导拿走。
    pub badge: Option<String>,
    /// 吉祥物的反应，引导拿走。
    pub mood: Option<Mood>,
    existing: Option<Existing>,
    entered: bool,
    in_editor: Option<Slot>,
}

impl Default for PersonaStep {
    fn default() -> Self {
        Self {
            name: Field::line(),
            avatar: Field::line(),
            avatar_blob: None,
            prompt: Field::lines(),
            reminders: Field::lines(),
            pairs: Vec::new(),
            sel: 0,
            listing: false,
            editing: None,
            writing: None,
            focus: Slot::Name,
            busy: false,
            error: None,
            existing_name: None,
            chosen: None,
            chosen_id: None,
            badge: None,
            mood: None,
            existing: None,
            entered: false,
            in_editor: None,
        }
    }
}

impl PersonaStep {
    /// 进这一步：第一次进的先看有没有默认人格。
    pub fn enter(&mut self, asker: &mut Asker) {
        if self.entered {
            return;
        }
        self.entered = true;
        self.busy = true;
        asker.ask(
            "config.get",
            json!({"keys": ["persona.default"]}),
            Waiting::PersonaDefault,
        );
    }

    /// 光标那一格是打字的：交回它。
    fn field(&mut self, slot: Slot) -> Option<&mut Field> {
        match slot {
            Slot::Name => Some(&mut self.name),
            Slot::Avatar => Some(&mut self.avatar),
            Slot::Prompt => Some(&mut self.prompt),
            Slot::Reminders => Some(&mut self.reminders),
            Slot::Examples | Slot::Next => None,
        }
    }

    /// 按了一个键。
    pub fn key(
        &mut self,
        key: KeyEvent,
        asker: &mut Asker,
        texts: &Texts,
        effects: &mut Vec<Effect>,
    ) -> Move {
        if let Some((slot, mut sheet)) = self.writing.take() {
            match sheet.key(key) {
                SheetKey::Close => {
                    if let Some(field) = self.field(slot) {
                        *field = Field::lines().with(sheet.text());
                    }
                    return Move::Stay;
                }
                SheetKey::External => {
                    self.in_editor = Some(slot);
                    effects.push(Effect::Editor(sheet.text().to_string()));
                }
                SheetKey::Stay => {}
            }
            self.writing = Some((slot, sheet));
            return Move::Stay;
        }
        if let Some(mut draft) = self.editing.take() {
            match pairs::form_key(&mut draft.sides, &mut draft.focus, key) {
                FormKey::Cancel => {}
                FormKey::Missing => {
                    draft.missing = true;
                    self.editing = Some(draft);
                }
                FormKey::Done(pair) => {
                    pairs::put(&mut self.pairs, &mut self.sel, draft.index, pair)
                }
                FormKey::Stay => self.editing = Some(draft),
            }
            return Move::Stay;
        }
        if self.listing {
            match pairs::list_key(&mut self.pairs, &mut self.sel, key) {
                ListKey::Close => self.listing = false,
                ListKey::Edit(index) => {
                    let pair = index.map(|i| self.pairs[i].clone()).unwrap_or_default();
                    self.editing = Some(PairDraft {
                        sides: pairs::opened(&pair),
                        focus: 0,
                        index,
                        missing: false,
                    });
                }
                ListKey::Changed | ListKey::Stay => {}
            }
            return Move::Stay;
        }
        if self.busy {
            return if key.code == KeyCode::Esc {
                Move::Back
            } else {
                Move::Stay
            };
        }
        let at = SLOTS.iter().position(|s| *s == self.focus).unwrap_or(0);
        // 名称、头像那一格 `Enter` 才开始编辑，编辑时 `Enter` 写好、`Esc` 不改；别的时候都认 vim 的键（第 7 条）。
        if self.name.editing() {
            if self.name.edit(key) == Edit::Done {
                self.error = None;
            }
            return Move::Stay;
        }
        if self.avatar.editing() {
            if self.avatar.edit(key) == Edit::Done {
                self.error = None;
                self.settle_avatar();
            }
            return Move::Stay;
        }
        let moved = nav::plain(key);
        match key.code {
            KeyCode::Esc => return Move::Back,
            _ if moved == Some(Nav::Up) => self.focus = SLOTS[at.saturating_sub(1)],
            _ if moved == Some(Nav::Down) => self.focus = SLOTS[(at + 1).min(SLOTS.len() - 1)],
            KeyCode::Enter => match self.focus {
                Slot::Prompt | Slot::Reminders => {
                    let text = self
                        .field(self.focus)
                        .map(|f| f.text().to_string())
                        .unwrap_or_default();
                    self.writing = Some((self.focus, Sheet::open(&text)));
                }
                Slot::Examples => {
                    self.listing = true;
                    self.sel = 0;
                }
                Slot::Next => self.save(asker, texts),
                Slot::Name => self.name.begin(),
                Slot::Avatar => self.avatar.begin(),
            },
            _ => {}
        }
        Move::Stay
    }

    /// 粘贴：开着的浮窗收（大编辑浮窗、两格窗光标那一格），都没开的进名字那一格。
    pub fn paste(&mut self, text: &str) {
        if let Some((_, sheet)) = self.writing.as_mut() {
            sheet.paste(text);
        } else if let Some(draft) = self.editing.as_mut() {
            draft.sides[draft.focus].insert(text);
        } else if !self.listing && self.focus == Slot::Name {
            if !self.name.editing() {
                self.name.begin();
            }
            self.name.paste(text);
        } else if !self.listing && self.focus == Slot::Avatar {
            // 拖进来的路径：去掉两头的引号（kitty 不加，别的终端多半加）。
            if !self.avatar.editing() {
                self.avatar.begin();
            }
            self.avatar.paste(text.trim().trim_matches(['\'', '"']));
        }
    }

    /// 头像的路径写好了：和传上去的不是同一个的，传上去的作废，存的时候重传。
    fn settle_avatar(&mut self) {
        self.avatar_blob = None;
    }

    /// 头像那一格指着的文件（`~/`、相对路径照终端所在的目录接成绝对的）；空着的是 `None`。
    pub fn avatar_path(&self) -> Option<std::path::PathBuf> {
        let text = self.avatar.trimmed();
        let text = text.trim_matches(['\'', '"']);
        if text.is_empty() {
            return None;
        }
        let cwd = std::env::current_dir().unwrap_or_default();
        Some(crate::local::path(
            text,
            std::env::home_dir().as_deref(),
            &cwd,
        ))
    }

    /// 已有的人格有头像。
    pub fn has_avatar(&self) -> bool {
        self.existing.as_ref().is_some_and(|e| e.avatar.is_some())
    }

    /// 外部编辑器退出了：写回大编辑浮窗（接着在浮窗里，没改、读不回来的不动）。
    pub fn edited(&mut self, text: Option<String>) {
        let slot = self.in_editor.take();
        if let (Some(slot), Some(text)) = (slot, text)
            && let Some((open, sheet)) = self.writing.as_mut()
            && *open == slot
        {
            sheet.replace(text.trim_end());
        }
    }
}

mod save;
