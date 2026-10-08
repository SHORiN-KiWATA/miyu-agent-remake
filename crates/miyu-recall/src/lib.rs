//! 检索的纯逻辑（`docs/blueprint/recall.md`，施工 R-1）：记忆、以后的知识库按关键词找东西用的底子。
//!
//! 第 2 层，纯逻辑，不碰 I/O：库在 `miyu-store` 的 `recall` 里，这里只管一段字怎么切成词。
//!
//! - [`index_terms`]：存进 FTS5 那一列的词；
//! - [`query`]：拼成 FTS5 的 `MATCH` 写法。
//!
//! 中文、日文没有空格，SQLite 自带的 `unicode61` 会把一整句当一个词，`trigram` 搜不到两个字的词，所以连着的汉字、假名
//! 照 Lucene 的 CJKBigram 两两切（`docs/reviews/2026-10-07-记忆知识库embedding调研.md` 第三节）。

mod memory;
mod terms;
mod turns;

pub use memory::{
    CLASSES, Cleared, Entry, MemoryBook, MemoryEvent, MemoryId, Retired, Saved, Source, from_event,
    to_event,
};
pub use terms::{MAX_QUERY_TERMS, index_terms, query};
pub use turns::{Change, TurnFeed, TurnItem, key, replay};
