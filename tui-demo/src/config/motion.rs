//! 压缩那一行的进度条怎么动（`layout.json` 的 `compaction`，蓝图 `tui.md`「正文」第 9 条）。

use serde::Deserialize;

/// 进度条按整格一顿一顿地追真实的字数、最前面那格闪、过一会儿整行呼吸。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompactionMotion {
    /// 真实的字数够多亮几格了，随机停多久再追，毫秒，`[最短, 最长]`。
    pub pause_ms: [u64; 2],
    /// 一下多亮几格，`[最少, 最多]`（不超过真实的字数够的格数）。
    pub step_cells: [u64; 2],
    /// 没压好之前最多到百分之几。
    pub cap_percent: u64,
    /// 最前面亮着的那一格一秒明暗来回几次。
    pub blink_hz: f64,
    /// 那一格最暗时是强调色的几成。
    pub blink_low: f64,
    /// 这次压缩过了多久开始呼吸，毫秒。
    pub breathe_after_ms: u64,
    /// 呼吸一个来回多久，毫秒。
    pub breathe_period_ms: u64,
    /// 呼吸最深时往白里偏多少，0 到 1。
    pub breathe_lift: f64,
}
