//! 检索的纯逻辑（`docs/blueprint/recall.md`，施工 R-1）：记忆、以后的知识库按关键词找东西用的底子。
//!
//! 第 2 层，纯逻辑，不碰 I/O：库在 `miyu-store` 的 `recall` 里，这里只管一段字怎么切成词。
//!
//! - [`index_terms`]：存进 FTS5 那一列的词；
//! - [`query`]：拼成 FTS5 的 `MATCH` 写法；
//! - [`embedding`]：本机 embedding 模型的清单（施工 R-5 中，小程序 `miyu-embed` 和核心都照它）；
//! - [`vector`]、[`fuse()`]：向量的写法、两路合并（施工 R-5 下）；
//! - [`extract`]、[`merge`]：抽取、合并交回来的怎么读（施工 R-6 上、R-7 上）；
//! - [`associate`]：联想怎么挑、门槛表（施工 R-8）。
//!
//! 中文、日文没有空格，SQLite 自带的 `unicode61` 会把一整句当一个词，`trigram` 搜不到两个字的词，所以连着的汉字、假名
//! 照 Lucene 的 CJKBigram 两两切（`docs/reviews/2026-10-07-记忆知识库embedding调研.md` 第三节）。

pub mod associate;
pub mod embedding;
pub mod extract;
mod fuse;
mod memory;
pub mod merge;
pub mod redact;
mod terms;
mod turns;
pub mod vector;

pub use fuse::{FLOOR, fuse};
pub use memory::{
    CLASSES, Cleared, Entry, Extracted, MemoryBook, MemoryEvent, MemoryId, Merged, Retired, Saved,
    Skipped, Source, Summary, from_event, to_event,
};
pub use terms::{MAX_QUERY_TERMS, index_terms, query};
pub use turns::{Change, TurnFeed, TurnItem, key, replay};
