//! `/language`：开一个框选界面语言（蓝图 `tui.md`「界面语言」）。第一行是自动（跟随系统），下面一种一行是手动选。
//! `↑` `↓` 选，`Enter` 换成选中的那一档，`Esc` 关；鼠标悬停选中、点一下等于 `Enter`。换了以后界面上的字、命令的说明、运行状态行的词、工具的显示名照新语言；已经画在正文里的不改，
//! 新画的照新语言。选的写进个人设置的 `ui.language`；启动时、别的头改了照配置换（2026-10-01 项目主人定 A）。

use ratatui::crossterm::event::{KeyCode, KeyEvent, MouseEvent, MouseEventKind};
use ratatui::layout::Position;

use super::{App, Panel};
use crate::config::Config;
use crate::core::Command;
use crate::language::Language;

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
        self.switch_language(index, true);
    }

    /// 换成第 `index` 行那一档并写进个人设置；`announce` 时提示一句（引导里换的不提示）。
    pub(super) fn switch_language(&mut self, index: usize, announce: bool) {
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
        // 写进个人设置的 `ui.language`：这个人所有的头都照它（2026-10-01 项目主人定 A）。当场换，不等回应。
        let code = if auto { "auto" } else { next.code() };
        self.core.send(Command::SetLanguage(code.to_string()));
        self.use_language(next, auto, announce);
    }

    /// 配置里的界面语言（连上时读的、别处改了推来的）：`auto` 跟系统，表里认得的换成那种，认不得的当 `auto`。
    /// 不写回去、不弹提示（启动时、别的头改的都悄悄换）。
    pub(super) fn language_from_config(&mut self, code: &str) {
        let table = &self.config.language_table;
        let (next, auto) = match table.find(code) {
            Some(language) if code != "auto" => (language, false),
            _ => (self.system_language.clone(), true),
        };
        if next == self.config.language && auto == self.config.auto {
            return;
        }
        self.use_language(next, auto, false);
    }

    /// 换成 `next`（`auto`：是自动那一档）：界面上的字、命令的说明、运行状态行的词、工具的显示名照它；`announce`
    /// 时提示一句换成了哪种。
    fn use_language(&mut self, next: Language, auto: bool, announce: bool) {
        let Ok(fresh) = Config::load(&next, auto) else {
            return;
        };
        self.config.text = fresh.text;
        self.config.commands = fresh.commands;
        self.config.pulse = fresh.pulse;
        self.core.send(Command::FetchHuman(next.code().to_string()));
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
        // 排好的行里有旧语言的字（时间线的标题、收起那一行、工具的显示名）：语言算进排版的条件，下一帧照预算重排，
        // 不扔掉记着的行（没轮到的先用旧行，蓝图「正文」第 8 条）。
        if announce {
            self.hint(hint, false);
        }
    }
}
