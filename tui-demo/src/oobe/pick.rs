//! 选一行的一步（语言、图标，「第一次打开的引导」第 13、14 条）：一行行字、选着哪一行。

use ratatui::crossterm::event::KeyEvent;

use super::nav::{self, Nav};

/// 选一行。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pick {
    /// 一行行。
    pub rows: Vec<String>,
    /// 选着第几行。
    pub selected: usize,
}

impl Pick {
    /// 这几行，选着第 `selected` 行（超出的停在最后一行）。
    pub fn new(rows: Vec<String>, selected: usize) -> Self {
        let selected = selected.min(rows.len().saturating_sub(1));
        Self { rows, selected }
    }

    /// 上下挪（vim 的 `j` `k` 也认）：挪了交回 `true`，别的键交回 `false`。
    pub fn key(&mut self, key: KeyEvent) -> bool {
        let last = self.rows.len().saturating_sub(1);
        match nav::plain(key) {
            Some(Nav::Up) => self.selected = self.selected.saturating_sub(1),
            Some(Nav::Down) => self.selected = (self.selected + 1).min(last),
            _ => return false,
        }
        true
    }
}
