//! 策略快照（`docs/designs/03-事件模型.md` E5，施工 3-6 上）：一个会话发请求要用的全部字和几样开关，
//! 按内容哈希存档，任何时候回放都能逐字节重现当时的请求。
//!
//! 第 2 层，纯逻辑。读文件是执行器的事（`miyu-store` 的资源目录），这里收读好的原文 [`Sources`]：
//!
//! - [`compose()`]：拼成 [`Snapshot`]，system 照 `docs/designs/26-提示词.md` 第四节拼；
//!   [`Snapshot::with_tools`] 带上工具面（施工 4-1），[`ToolEntry::offer`] 照会话开局时的配置填一个参数能选的几个（施工
//!   8-8 补）；
//! - [`Snapshot::to_bytes`]、[`Snapshot::hash`]、[`Snapshot::from_bytes`]：规范的字节、内容哈希、读回来；
//! - [`Snapshot::policy`]、[`Snapshot::driver_texts`]：照快照造出内核的策略、驱动的占位。

mod compose;
mod drivers;
mod facts;
mod guard;
mod harness;
mod image_name;
mod jobs;
pub mod memory;
mod pause;
mod peers;
pub mod persona;
mod rebuild;
mod recap;
mod shorten;
mod snapshot;
mod text_file;
mod title;
mod tools;
mod vision;

#[cfg(test)]
mod test_support;

pub use compose::{CoreLines, PersonaTexts, Sources, Wrap, compose};
pub use drivers::DriverPlaceholders;
pub use facts::FactTexts;
pub use guard::GuardTexts;
pub use harness::HarnessTexts;
pub use image_name::ImageNameTexts;
pub use jobs::{DEPTH as JOB_DEPTH, JobNumbers, JobTexts, REPORT_CHARS};
pub use pause::{PAUSE, PauseNumbers};
pub use peers::{PEERS, PeerIdleTexts, PeerNumbers, PeerTexts};
pub use rebuild::{REBUILD, RebuildNumbers, RebuildTexts};
pub use recap::{RECAP, RecapNumbers, RecapTexts};
pub use shorten::{SHORTEN, ShortenNumbers, ShortenTexts};
pub use snapshot::{
    BuildError, CompactionNumbers, CompactionTexts, CoreTexts, PermissionTexts, Snapshot,
    SnapshotError, ToolResultTexts, TurnEndedTexts,
};
pub use text_file::TextFileTexts;
pub use title::{TITLE, TitleNumbers, TitleTexts};
pub use tools::{Choice, RunTexts, ToolEntry};
pub use vision::{ImageDescriptionTexts, VisionTexts};
