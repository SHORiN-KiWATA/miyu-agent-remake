//! `@` 文件列表（蓝图 `tui.md`「`@` 文件列表」）：光标前面是 `@` 打头的词时开，词变了问核心一次（`ask.rs`），交出列哪些。
//! 开没开、选中哪一条、`Esc` 关掉以后什么时候再开、问到哪了，也记在这里；收发不在这里（`app/mention.rs`）。
//! 画在 `ui/mention.rs`。

mod ask;
mod token;

#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::config::MentionLook;
pub use ask::Ask;
pub use token::escape;

/// 一个词：`@` 在第几个字节、`@` 后面打的字。问核心、认回应都照它。
pub type Word = (usize, String);

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
    /// 列的是这个词的回应；`false` 时还没回，列的是上一个词的（或者空着），不写「没有对得上的」。
    pub ready: bool,
}

/// 核心回来的一份：哪个词问的、列哪些、怎么找来的、清单还在不在建。
#[derive(Debug)]
struct Reply {
    word: Word,
    items: Vec<Candidate>,
    status: Status,
    building: bool,
}

/// 列表的状态。
#[derive(Debug)]
pub struct Mention {
    /// 选中的是第几条。
    pub selected: usize,
    /// 鼠标钉住的露出来的那一段（`menu::pin`）；按键选、词变了放开。
    pub pinned: Option<usize>,
    look: MentionLook,
    /// 现在的词：变了，选中回到第一条。
    typed: Option<Word>,
    /// `Esc` 关掉时的词：没变就一直关着。
    dismissed: Option<Word>,
    /// 刚弹出来、还没问过：这一次模糊找写 `fresh`。
    opened: bool,
    /// 最近一次问的词、什么时候问的、回应到了没有。
    asked: Option<(Word, Instant, bool)>,
    /// 最近一份用得上的回应。
    reply: Option<Reply>,
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
            opened: false,
            asked: None,
            reply: None,
        }
    }

    /// 照输入框里的字和光标，定开不开、列哪些；不开的是 `None`。只看记着的回应，不收发。
    pub fn find(&mut self, text: &str, cursor: usize) -> Option<Found> {
        let Some(word) = token::token(text, cursor) else {
            self.typed = None;
            self.dismissed = None;
            self.asked = None;
            self.reply = None;
            return None;
        };
        if self.typed.as_ref() != Some(&word) {
            self.opened |= self.typed.is_none();
            self.typed = Some(word.clone());
            self.selected = 0;
            self.pinned = None;
        }
        if self.dismissed.as_ref() == Some(&word) {
            return None;
        }
        self.dismissed = None;
        let (start, query) = word;
        let (items, status, ready) = match &self.reply {
            Some(reply) => {
                let ready = reply.word.0 == start && reply.word.1 == query;
                let status = if ready {
                    reply.status
                } else {
                    Self::waiting(&query)
                };
                (reply.items.clone(), status, ready)
            }
            None => (Vec::new(), Self::waiting(&query), false),
        };
        self.selected = self.selected.min(items.len().saturating_sub(1));
        Some(Found {
            start,
            query,
            items,
            status,
            ready,
        })
    }

    /// 这个词还没回时框上写哪一句。
    fn waiting(query: &str) -> Status {
        match Ask::of(query, false) {
            Ask::List { .. } => Status::Layer,
            Ask::Find { .. } => Status::Indexing,
        }
    }

    /// 现在该不该问核心：开着、词没问过，或者这个词的清单还在建、上一次回来了、隔了 `poll_ms`。问了就记下。
    pub fn next_ask(&mut self, now: Instant) -> Option<(Word, Ask)> {
        let word = self.typed.clone()?;
        if self.dismissed.as_ref() == Some(&word) {
            return None;
        }
        let again = match &self.asked {
            Some((asked, _, _)) if *asked != word => true,
            None => true,
            Some(_) => self.due().is_some_and(|at| now >= at),
        };
        if !again {
            return None;
        }
        let ask = Ask::of(&word.1, std::mem::take(&mut self.opened));
        self.asked = Some((word.clone(), now, false));
        Some((word, ask))
    }

    /// 这个词的清单还在建、上一次回来了：几时再问。
    fn due(&self) -> Option<Instant> {
        let (asked, at, back) = self.asked.as_ref()?;
        let reply = self.reply.as_ref()?;
        (*back && reply.building && reply.word == *asked && self.typed.as_ref() == Some(asked))
            .then(|| *at + Duration::from_millis(self.look.poll_ms))
    }

    /// 下一次要自己醒来再问的时刻。
    pub fn deadline(&self) -> Option<Instant> {
        self.due()
    }

    /// 核心回了 `word` 问的（回了错的是 `None`）：是现在这个词的才用。
    pub fn replied(&mut self, word: &Word, result: Option<&Value>) {
        if let Some((asked, _, back)) = &mut self.asked
            && asked == word
        {
            *back = true;
        }
        if self.typed.as_ref() != Some(word) {
            return;
        }
        let (items, status, building) = Ask::of(&word.1, false).read(result);
        self.reply = Some(Reply {
            word: word.clone(),
            items,
            status,
            building,
        });
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
}
