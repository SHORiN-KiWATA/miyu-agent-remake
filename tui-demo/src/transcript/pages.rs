//! 老会话按页读（蓝图 `tui.md`「会话列表 `/sessions`」第 5 条「按页读」，核心 9-6）：更早的还有时正文最上面一行暗色的
//! 「正在读更早的…」；更早的一页读成一份临时的正文，拼在前面；订阅回应里的累计用量、权限整个换掉页里算出来的。

use std::collections::HashMap;

use super::{Entry, Kind, Slot, Transcript};
use crate::config::Texts;
use crate::core::{Level, Spent};

/// 按页读读到哪了。
#[derive(Debug, Default)]
pub struct Older {
    /// 更早的还有：最上面那一行在。
    more: bool,
    /// 要了更早的一页，还没回来：不再要。
    loading: bool,
    /// 每一轮最后是撤掉（`true`）还是恢复：更早的页拼上来时照它藏起、显示回来（新的页说了算）。
    undone: HashMap<u64, bool>,
}

impl Transcript {
    /// 最新一页读进来了，或者更早的一页拼好了：更早的还有的，最上面那一行在，不然去掉。
    pub fn paged(&mut self, more: bool, texts: &Texts) {
        self.older.more = more;
        self.older.loading = false;
        let marked = self.entries.first().is_some_and(|e| e.kind == Kind::Older);
        if more && !marked {
            let id = self.fresh_id();
            self.entries
                .insert(0, Entry::new(id, Kind::Older, texts.older.clone()));
            self.shift(1);
        } else if !more && marked {
            self.entries.remove(0);
            self.removed(0);
        }
    }

    /// 该要更早的一页了：最上面那一行在、还没要。
    pub fn wants_older(&self) -> bool {
        self.older.more && !self.older.loading
    }

    /// 要了更早的一页。
    pub fn asked_older(&mut self) {
        self.older.loading = true;
    }

    /// 撤掉、恢复了这几轮：记下最后是哪样。
    pub(super) fn decided(&mut self, turns: &[u64], hidden: bool) {
        for &turn in turns {
            self.older.undone.insert(turn, hidden);
        }
    }

    /// 读更早的一页用的一份空正文：会话、连接、权限级别照这一份。
    pub fn history(&self) -> Transcript {
        Transcript {
            session: self.session.clone(),
            link: self.link.clone(),
            level: self.level,
            ..Transcript::default()
        }
    }

    /// 把读成的更早的一页（[`Transcript::history`] 那一份）拼在前面：编号重发、不和已有的撞；新的页撤掉、恢复过的轮照
    /// 新的页藏起、显示回来；更早的还有没有照 `more`。交回拼进来几条（不算最上面那一行）。用量、缓存那几样不加：
    /// 订阅回应里给的是整个会话的。
    pub fn prepend(&mut self, mut older: Transcript, more: bool, texts: &Texts) -> usize {
        if self.entries.first().is_some_and(|e| e.kind == Kind::Older) {
            self.entries.remove(0);
            self.removed(0);
        }
        older.finish_segment();
        let base = self.next_id;
        for entry in &mut older.entries {
            entry.id += base;
        }
        self.next_id += older.next_id;
        let n = older.entries.len();
        self.shift(n);
        for (call, at) in std::mem::take(&mut older.calls) {
            self.calls.entry(call).or_insert(at);
        }
        for (&turn, &hidden) in &self.older.undone {
            older.hide(&[turn], hidden);
        }
        for (turn, hidden) in older.older.undone {
            self.older.undone.entry(turn).or_insert(hidden);
        }
        older.entries.append(&mut self.entries);
        self.entries = older.entries;
        self.paged(more, texts);
        n
    }

    /// 前面插进来 `by` 条：记着的第几条都往后挪。
    fn shift(&mut self, by: usize) {
        for slot in self.blocks.values_mut() {
            match slot {
                Slot::Reply(i) | Slot::Step(i, _) => *i += by,
            }
        }
        for (i, _) in self.unnamed.iter_mut().chain(self.calls.values_mut()) {
            *i += by;
        }
        if let Some(i) = self.compacting.as_mut() {
            *i += by;
        }
    }

    /// 订阅回应里的累计用量、权限（核心 9-6 上）：整个换掉页里的事件算出来的，之后照推来的往上加。没有的那一样不动。
    pub fn snapshot(&mut self, spent: Option<&Spent>, level: Option<Level>) {
        if let Some(spent) = spent {
            self.total = spent.total;
            self.bill = spent.bill.clone();
            self.cache.compactions = spent.compactions;
            self.cache.breaks = spent.breaks;
        }
        if let Some(level) = level {
            self.level = level;
        }
    }
}

#[cfg(test)]
mod tests;
