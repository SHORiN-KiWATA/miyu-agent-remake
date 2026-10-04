//! 帮助框的按键和滚轮（蓝图 `tui.md`「帮助 `/help`」）：`↑` `↓` 一行一行滚，`PgUp` `PgDn` 一页一页滚，滚轮在框上也滚，
//! `Esc` 关；开着时按键归它。

use ratatui::crossterm::event::{KeyCode, KeyEvent, MouseEvent, MouseEventKind};
use ratatui::layout::Position;

use super::{App, Panel};

impl App {
    /// 打开帮助框。
    pub(super) fn open_help(&mut self) {
        self.panel = Some(Panel::Help { scroll: 0 });
    }

    /// 帮助框开着时的按键。
    pub(super) fn help_key(&mut self, scroll: usize, key: KeyEvent) {
        let page = self.panel_rows.len().max(1);
        let next = match key.code {
            KeyCode::Up => scroll.saturating_sub(1),
            KeyCode::Down => scroll + 1,
            KeyCode::PageUp => scroll.saturating_sub(page),
            KeyCode::PageDown => scroll + page,
            KeyCode::Esc => {
                self.panel = None;
                return;
            }
            _ => return,
        };
        self.scroll_help(next);
    }

    /// 滚轮在帮助框上：滚一行；交回归不归它。
    pub(super) fn help_mouse(&mut self, mouse: MouseEvent, at: Position) -> bool {
        let Some(Panel::Help { scroll }) = self.panel else {
            return false;
        };
        if !self.areas.menu.contains(at) {
            return false;
        }
        match mouse.kind {
            MouseEventKind::ScrollUp => self.scroll_help(scroll.saturating_sub(1)),
            MouseEventKind::ScrollDown => self.scroll_help(scroll + 1),
            _ => {}
        }
        true
    }

    /// 滚到第 `to` 行起，滚过头停在最后一屏。
    fn scroll_help(&mut self, to: usize) {
        let total = crate::ui::help::count(&self.config, self.areas.menu_text.width);
        let shown = self.panel_rows.len().max(1);
        let scroll = to.min(total.saturating_sub(shown));
        self.panel = Some(Panel::Help { scroll });
    }
}
