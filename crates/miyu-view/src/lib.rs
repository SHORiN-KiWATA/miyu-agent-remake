//! 视图投影（`docs/blueprint/view.md`，设计 `04-核心协议.md` P3）：会话的事件算成头直接画的一条条条目。核心决定
//! 「显示什么」：种类、状态、属于哪一轮、标题那一句、收起那一行；头决定「怎么画」。
//!
//! 纯逻辑：不碰磁盘、网络、时钟。事件一条条喂进 [`Projector`]，每喂一条交回它引起的变化（[`Change`]）；给人看的字
//! （[`Texts`]）、改了多少行（[`Lines`]）由外面交进来。

mod change;
mod entry;
mod estimate;
mod explain;
mod id;
mod notice;
mod projector;
mod summary;
mod title;
mod words;

/// 视图投影的版本：握手时报给头（`docs/blueprint/view.md`「握手」）。条目的格只加不改；改了已有的格才加一。
pub const VERSION: u32 = 1;

pub use change::Change;
pub use entry::{
    Answered, Approval, Attachment, Body, Diff, End, Entry, EntryId, Group, JobDone, JobEnd, Mark,
    ModelSwap, Output, Part, Peer, Picture, Reply, Thought, Title, Tone, Tool, ToolState, User,
    Was,
};
pub use notice::{Compaction, CompactionState, Notice, RevertedFile};
pub use projector::{Lines, Projector};
pub use words::{Face, Kinds, Texts, ToolKind, Words};
