//! 「更多供应商…」浮窗（蓝图 `tui.md`「第一次打开的引导」第 16a 条，2026-10-10 项目主人）：开窗发一次不带 `featured` 的
//! `provider.catalog` 拿全目录，本地筛。开窗就在搜索那一格里，打字就筛（名字、编号里有打的字的，不分大小写），照名字排；
//! 接不上的暗着、停不上；`Enter` 交回选的那一家，进填写那一屏（同在列表里选了它）。画在 `ui/oobe/more.rs`。

use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::rows::Provider;
use crate::oobe::field::Field;
use crate::oobe::nav::{self, Nav};

/// 按了一个键以后要做的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MoreKey {
    /// 接着开着。
    Stay,
    /// 关窗回列表。
    Close,
    /// 选了这一家。
    Pick(Provider),
}

/// 浮窗。
#[derive(Debug)]
pub struct More {
    /// 搜索那一格：开窗就在编辑。
    pub filter: Field,
    /// 全目录；回来以前是 `None`。
    pub all: Option<Vec<Provider>>,
    /// 光标停在对得上的第几家。
    pub cursor: usize,
}

impl More {
    /// 开窗：搜索那一格空着、在编辑。
    pub fn open() -> Self {
        Self {
            filter: Field::line().live(),
            all: None,
            cursor: 0,
        }
    }

    /// 全目录回来了：照名字排，光标停到第一家接得上的。
    pub fn loaded(&mut self, mut all: Vec<Provider>) {
        all.sort_by_key(|p| p.name.to_lowercase());
        self.all = Some(all);
        self.cursor = self.first();
    }

    /// 对得上搜索的几家，照名字排。
    pub fn matches(&self) -> Vec<&Provider> {
        let query = self.filter.text().trim().to_lowercase();
        self.all
            .iter()
            .flatten()
            .filter(|p| {
                query.is_empty()
                    || p.name.to_lowercase().contains(&query)
                    || p.catalog.to_lowercase().contains(&query)
            })
            .collect()
    }

    /// 按了一个键：上下挪（跳过接不上的），`Enter` 选，`Esc` 关，别的照打字筛。
    pub fn key(&mut self, key: KeyEvent) -> MoreKey {
        match (key.code, nav::search(key)) {
            (_, Some(Nav::Up)) => self.cursor = self.step(false),
            (_, Some(Nav::Down)) => self.cursor = self.step(true),
            (KeyCode::Esc, _) => return MoreKey::Close,
            (KeyCode::Enter, _) => {
                if let Some(p) = self.matches().get(self.cursor).filter(|p| p.supported) {
                    return MoreKey::Pick((*p).clone());
                }
            }
            _ => {
                if self.filter.key(key) {
                    self.cursor = self.first();
                }
            }
        }
        MoreKey::Stay
    }

    /// 粘贴进搜索那一格。
    pub fn paste(&mut self, text: &str) {
        self.filter.paste(text);
        self.cursor = self.first();
    }

    /// 对得上的里头第一家接得上的。
    fn first(&self) -> usize {
        self.matches().iter().position(|p| p.supported).unwrap_or(0)
    }

    /// 往下（`down`）、往上挪到下一家接得上的；没有的停在原地。
    fn step(&self, down: bool) -> usize {
        let found = self.matches();
        let mut at = self.cursor;
        loop {
            at = if down {
                at + 1
            } else {
                match at.checked_sub(1) {
                    Some(at) => at,
                    None => return self.cursor,
                }
            };
            match found.get(at) {
                Some(p) if p.supported => return at,
                Some(_) => {}
                None => return self.cursor,
            }
        }
    }
}
