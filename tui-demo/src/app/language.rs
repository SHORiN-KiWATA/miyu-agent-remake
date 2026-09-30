//! `/language`：开一个框选界面语言（蓝图 `tui.md`「界面语言」）。第一行是自动（跟随系统），下面一种一行是手动选。
//! `↑` `↓` 选，`Enter` 换成选中的那一档，`Esc` 关；鼠标悬停选中、点一下等于 `Enter`。换了以后界面上的字、命令的说明、运行状态行的词、工具的显示名照新语言；已经画在正文里的不改，
//! 新画的照新语言。只管这一次启动。

use ratatui::crossterm::event::{KeyCode, KeyEvent, MouseEvent, MouseEventKind};
use ratatui::layout::Position;

use super::{App, Panel};
use crate::config::Config;

impl App {
    /// 打开框，选中现在用的那一档：自动是第 0 行，手动的第几种是第几加一行。
    pub(super) fn open_languages(&mut self) {
        let table = &self.config.language_table;
        let selected = if self.config.auto {
            0
        } else {
            table.position(&self.config.language).map_or(0, |i| i + 1)
        };
        self.panel = Some(Panel::Language { selected });
    }

    /// 框开着时的按键。
    pub(super) fn language_key(&mut self, selected: usize, key: KeyEvent) {
        // 第 0 行是自动，下面每种一行。
        let last = self.config.language_table.languages.len();
        match key.code {
            KeyCode::Up => {
                self.panel = Some(Panel::Language {
                    selected: selected.saturating_sub(1),
                });
            }
            KeyCode::Down => {
                self.panel = Some(Panel::Language {
                    selected: (selected + 1).min(last),
                });
            }
            KeyCode::Enter => self.choose_language(selected),
            KeyCode::Esc => self.panel = None,
            _ => {}
        }
    }

    /// 鼠标在框上：悬停选中，点一下换；交回归不归它。点在框的边上不算点中哪一种。
    pub(super) fn language_mouse(&mut self, mouse: MouseEvent, at: Position) -> bool {
        let Some(Panel::Language { .. }) = self.panel else {
            return false;
        };
        let areas = self.areas;
        if !areas.menu.contains(at) {
            return false;
        }
        let row = at.y.checked_sub(areas.menu_text.y).map(usize::from);
        let index = row.and_then(|row| self.panel_rows.get(row).copied().flatten());
        if let Some(index) = index.filter(|_| areas.menu_text.contains(at)) {
            self.panel = Some(Panel::Language { selected: index });
            if matches!(mouse.kind, MouseEventKind::Down(_)) {
                self.choose_language(index);
            }
        }
        true
    }

    /// 换成第 `index` 行那一档（第 0 行自动，跟启动时认出来的系统语言），关框，提示一句；选的就是现在的只关框。
    fn choose_language(&mut self, index: usize) {
        self.panel = None;
        let table = &self.config.language_table;
        let (next, auto) = if index == 0 {
            (self.system_language.clone(), true)
        } else {
            let Some(language) = table.nth(index - 1) else {
                return;
            };
            (language, false)
        };
        if next == self.config.language && auto == self.config.auto {
            return;
        }
        let Ok(fresh) = Config::load(&next, auto) else {
            return;
        };
        self.config.text = fresh.text;
        self.config.commands = fresh.commands;
        self.config.pulse = fresh.pulse;
        self.human = next.human();
        let hint = if auto {
            let name = fresh.language_table.name(&next);
            self.config
                .text
                .languages
                .auto_switched
                .replace("{name}", name)
        } else {
            self.config.text.language_switched.clone()
        };
        self.config.language = next;
        self.config.auto = auto;
        // 排好的行里有旧语言的字（时间线的标题、收起那一行、工具的显示名）：重排。
        *self.row_cache.borrow_mut() = Default::default();
        self.hint(hint, false);
    }
}
