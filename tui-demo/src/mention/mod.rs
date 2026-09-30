//! `@` 文件列表（蓝图 `tui.md`「`@` 文件列表」）：光标前面是 `@` 打头的词时开，按目录找或者模糊找，交出列哪些。
//! 开没开、选中哪一条、`Esc` 关掉以后什么时候再开，也记在这里。画在 `ui/mention.rs`。

mod fuzzy;
mod index;
mod listing;
mod token;

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::config::MentionLook;
use index::Index;
pub use token::escape;

/// 列出来的一条。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// 显示的字：相对工作目录的路径，或者照打的写法（`~/Documents/`），目录后面带 `/`。
    pub shown: String,
    /// 真的位置。
    pub path: PathBuf,
    /// 是目录。
    pub dir: bool,
    /// `shown` 里对上的是第几个字（按字符数）。
    pub hits: Vec<usize>,
}

/// 列的是怎么找来的，框上照它写一句（第 2 条）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// 按目录找：只读这一层。
    Layer,
    /// 模糊找，清单还在建。
    Indexing,
    /// 模糊找，清单收满了，列的只是一部分。
    Partial,
    /// 模糊找，清单全。
    Full,
}

/// 这一次列的：`@` 在第几个字节、打的字、列哪些、怎么找来的。
#[derive(Debug, Clone)]
pub struct Found {
    /// `@` 在输入框的字里第几个字节：收成块时从这里换到光标。
    pub start: usize,
    /// `@` 后面打的字。
    pub query: String,
    /// 列哪些。
    pub items: Vec<Candidate>,
    /// 怎么找来的。
    pub status: Status,
}

/// 列表的状态。
#[derive(Debug)]
pub struct Mention {
    /// 选中的是第几条。
    pub selected: usize,
    /// 鼠标钉住的露出来的那一段（`menu::pin`）；按键选、词变了放开。
    pub pinned: Option<usize>,
    look: MentionLook,
    /// 上一次的词（`@` 在哪、打的字）：变了，选中回到第一条。
    typed: Option<(usize, String)>,
    /// `Esc` 关掉时的词：没变就一直关着。
    dismissed: Option<(usize, String)>,
    /// 清单和什么时候开始建的。
    index: Option<(Index, Instant)>,
    /// 上一次模糊找的：打的字、那时清单有几条、列出来的。清单没长、字没变就不重算。
    cache: Option<(String, usize, Vec<Candidate>)>,
}

impl Mention {
    /// 照配置（`mention.json`）。
    pub fn new(look: MentionLook) -> Self {
        Self {
            selected: 0,
            pinned: None,
            look,
            typed: None,
            dismissed: None,
            index: None,
            cache: None,
        }
    }

    /// 照输入框里的字和光标，定开不开、列哪些；不开的是 `None`。`cwd` 是工作目录，`home` 是家目录。
    pub fn find(
        &mut self,
        text: &str,
        cursor: usize,
        cwd: &Path,
        home: Option<&Path>,
    ) -> Option<Found> {
        let Some((start, query)) = token::token(text, cursor) else {
            self.typed = None;
            self.dismissed = None;
            return None;
        };
        let word = (start, query.clone());
        if self.typed.as_ref() != Some(&word) {
            // 刚弹出来（上一次没开）：清单旧了重建（第 2 条）。
            if self.typed.is_none() {
                self.refresh(cwd);
            }
            self.typed = Some(word.clone());
            self.selected = 0;
            self.pinned = None;
        }
        if self.dismissed.as_ref() == Some(&word) {
            return None;
        }
        self.dismissed = None;
        let (items, status) = if listing::by_layer(&query) {
            let items = listing::list(&query, cwd, home, self.look.shown);
            (items, Status::Layer)
        } else {
            self.fuzzy(&query, cwd)
        };
        self.selected = self.selected.min(items.len().saturating_sub(1));
        Some(Found {
            start,
            query,
            items,
            status,
        })
    }

    /// 往上、往下挪一条，到头就停；鼠标钉住的放开。
    pub fn step(&mut self, down: bool, count: usize) {
        self.pinned = None;
        self.selected = if down {
            (self.selected + 1).min(count.saturating_sub(1))
        } else {
            self.selected.saturating_sub(1)
        };
    }

    /// `Esc`：关掉，这个词没变就一直关着。
    pub fn dismiss(&mut self, found: &Found) {
        self.dismissed = Some((found.start, found.query.clone()));
    }

    /// 清单没有、或者建了超过 `refresh_secs` 的，重建一份。
    fn refresh(&mut self, cwd: &Path) {
        let stale = Duration::from_secs(self.look.refresh_secs);
        if self
            .index
            .as_ref()
            .is_none_or(|(_, at)| at.elapsed() >= stale)
        {
            self.index = Some((Index::start(cwd.to_path_buf(), &self.look), Instant::now()));
            self.cache = None;
        }
    }

    /// 模糊找：照清单建好的那部分筛、排，最多 `shown` 条。
    fn fuzzy(&mut self, query: &str, cwd: &Path) -> (Vec<Candidate>, Status) {
        if self.index.is_none() {
            self.refresh(cwd);
        }
        let Some((index, _)) = &self.index else {
            return (Vec::new(), Status::Indexing);
        };
        let (len, done, partial) = index.with(|b| (b.entries.len(), b.done, b.partial));
        let status = match (done, partial) {
            (false, _) => Status::Indexing,
            (true, true) => Status::Partial,
            (true, false) => Status::Full,
        };
        if let Some((q, n, items)) = &self.cache
            && q == query
            && *n == len
        {
            return (items.clone(), status);
        }
        let shown = self.look.shown;
        let items = index.with(|b| {
            let mut scored: Vec<(i64, Candidate)> = b
                .entries
                .iter()
                .filter_map(|(path, dir)| {
                    let shown = if *dir {
                        format!("{path}/")
                    } else {
                        path.clone()
                    };
                    let (score, hits) = fuzzy::score(query, &shown)?;
                    Some((
                        score,
                        Candidate {
                            shown,
                            path: cwd.join(path),
                            dir: *dir,
                            hits,
                        },
                    ))
                })
                .collect();
            scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.shown.cmp(&b.1.shown)));
            scored
                .into_iter()
                .take(shown)
                .map(|(_, c)| c)
                .collect::<Vec<_>>()
        });
        self.cache = Some((query.to_string(), len, items.clone()));
        (items, status)
    }
}
