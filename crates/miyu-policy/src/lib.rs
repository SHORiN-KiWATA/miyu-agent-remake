//! 策略快照（`docs/designs/03-事件模型.md` E5，施工 3-6 上）：一个会话发请求要用的全部字和几样开关，
//! 按内容哈希存档，任何时候回放都能逐字节重现当时的请求。
//!
//! 第 2 层，纯逻辑。读文件是执行器的事（`miyu-store` 的资源目录），这里收读好的原文 [`Sources`]：
//!
//! - [`compose()`]：拼成 [`Snapshot`]，system 照 `docs/designs/26-提示词.md` 第四节拼；
//!   [`Snapshot::with_tools`] 带上工具面（施工 4-1）；
//! - [`Snapshot::to_bytes`]、[`Snapshot::hash`]、[`Snapshot::from_bytes`]：规范的字节、内容哈希、读回来；
//! - [`Snapshot::policy`]、[`Snapshot::driver_texts`]：照快照造出内核的策略、驱动的占位。

mod compose;
mod drivers;
mod guard;
mod image_name;
mod jobs;
mod pause;
mod rebuild;
mod shorten;
mod snapshot;
mod text_file;
mod tools;

#[cfg(test)]
mod test_support;

pub use compose::{PersonaTexts, Sources, compose};
pub use drivers::DriverPlaceholders;
pub use guard::GuardTexts;
pub use image_name::ImageNameTexts;
pub use jobs::{DEPTH as JOB_DEPTH, JobNumbers, JobTexts, REPORT_CHARS};
pub use pause::{PAUSE, PauseNumbers};
pub use rebuild::{REBUILD, RebuildNumbers, RebuildTexts};
pub use shorten::{SHORTEN, ShortenNumbers, ShortenTexts};
pub use snapshot::{
    BuildError, CompactionNumbers, CompactionTexts, CoreTexts, FactTexts, PermissionTexts,
    Snapshot, SnapshotError, ToolResultTexts, TurnEndedTexts,
};
pub use text_file::TextFileTexts;
pub use tools::{RunTexts, ToolEntry};
