//! 用量的算法（蓝图 `tui.md`「配置与模型」第 5 条 `/usage`）：从 `usage.query` 交回的按天的一行行算单日最高、连续
//! 使用几天、热度图的格子和分档。只算，不画（画在 `ui/usage/`）。

mod days;

pub use days::{Day, days, grid, level, peak, streaks, thresholds};
