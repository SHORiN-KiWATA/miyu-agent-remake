//! `@` 文件列表的按键和鼠标（蓝图 `tui.md`「`@` 文件列表」第 5 条）：`↑`、`↓` 选，`Tab` 进目录（文件同 `Enter`），
//! `Enter` 收成块，`Esc` 关；悬停选中，点一下收成块。

use std::time::Instant;

use ratatui::crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};

use super::App;
use crate::core::Command;
use crate::input::Action;
use crate::mention::{Found, escape};

impl App {
    /// 这一帧开着的 `@` 列表：历史列表、后台面板、抽屉开着时不开。
    pub fn mention_found(&mut self) -> Option<Found> {
        // 翻输入历史翻出来的那句还没改过：不弹，`↑` `↓` 接着翻（蓝图「按键」`↑`、`↓`）。
        if self.history.open || self.panel.is_some() || self.drawers.open() || self.input.recalled()
        {
            return None;
        }
        let editor = &self.input.editor;
        let (text, cursor) = (editor.text().to_string(), editor.cursor());
        let found = self.mention.find(&text, cursor)?;
        // 词变了、清单在建到点了：问核心（`cwd` 是界面所在的工作目录）。
        if let Some((word, ask)) = self.mention.next_ask(Instant::now()) {
            let cwd = std::env::current_dir().unwrap_or_default();
            let (method, params) = ask.request(&cwd.to_string_lossy());
            self.core.send(Command::Files {
                word,
                method,
                params,
            });
        }
        Some(found)
    }

    /// 列表开着时的按键：管得了的交回 `Some`，别的（打字、挪光标）交给输入框。
    pub(super) fn mention_key(&mut self, key: KeyEvent, found: &Found) -> Option<Action> {
        match key.code {
            KeyCode::Up | KeyCode::Down => {
                self.mention
                    .step(key.code == KeyCode::Down, found.items.len());
            }
            KeyCode::Tab => self.mention_tab(found),
            KeyCode::Enter => self.mention_take(found),
            KeyCode::Esc => self.mention.dismiss(found),
            _ => return None,
        }
        Some(Action::None)
    }

    /// 列表里的鼠标（「斜杠命令列表」第 3 条）：悬停高亮指针下那一条、列表不动，滚轮滚，点一下收成块。
    pub(super) fn mention_mouse(&mut self, mouse: MouseEvent, found: &Found) -> Action {
        let rows = crate::ui::menu_rows(self.config.layout.menu_rows, self.areas.menu_text.height);
        let count = found.items.len();
        let Some(by) = super::mouse::list_step(mouse.kind, false) else {
            return Action::None;
        };
        let mention = &mut self.mention;
        crate::menu::pin(mention.selected, &mut mention.pinned, count, rows, by);
        let top = crate::menu::top(mention.selected, mention.pinned, count, rows);
        let text = self.areas.menu_text;
        let Some(index) = crate::ui::menu_index_at(text, count, top, rows, mouse.row) else {
            return Action::None;
        };
        self.mention.selected = index;
        if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
            self.mention_take(found);
        }
        Action::None
    }

    /// `Tab`：选中的是目录，词换成这个目录接着列；是文件，收成块。
    fn mention_tab(&mut self, found: &Found) {
        let Some(item) = found.items.get(self.mention.selected) else {
            return;
        };
        if item.dir {
            let text = format!("@{}", escape(&item.shown));
            self.input.retype_mention(found.start, &text);
        } else {
            self.mention_take(found);
        }
    }

    /// `Enter`、点一下：选中的那一条收成块。
    fn mention_take(&mut self, found: &Found) {
        if let Some(item) = found.items.get(self.mention.selected) {
            self.input.take_mention(found.start, item.path.clone());
        }
    }
}
