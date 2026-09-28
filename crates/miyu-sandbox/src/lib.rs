//! 沙盒（`docs/blueprint/sandbox.md`，施工 5-1 起）：核心给每条要关起来的命令写一份规格（[`Spec`]），小程序
//! `miyu-sandbox` 照规格先把自己收紧，再换成那条命令。助手是单独的一个小程序（`docs/designs/11-权限与沙盒.md` 第六节
//! A7）。
//!
//! - [`Spec`]：规格；[`Sandboxed`]：一次调用带的，助手在哪、规格是什么；
//! - [`argv`]：照规格把一条命令包成交给助手的样子；
//! - [`locate()`]：找助手，在主程序旁边；
//! - [`probe()`]：跑一次助手的 `probe`，读它说这台机器能收紧到什么程度（[`Probe`]、[`Platform`]）；
//! - [`install`]：装沙盒要管理员权限的那一次，Windows 上建、撤专用的沙盒用户（施工 5-8，`docs/blueprint/sandbox/windows.md`）；
//! - `testkit`：测试用的，cargo 编出来的助手在哪（施工 5-4 上）。
//!
//! 助手本身在 `src/bin/miyu-sandbox/`：共用的在 `main.rs`，收紧和换成命令各平台一个文件。
//!
//! Linux（Landlock，施工 5-2、5-3）、macOS（Seatbelt，施工 5-7）上照规格收紧；Windows 还不收紧（5-9）。

pub mod install;
mod locate;
mod probe;
mod spec;
#[cfg(feature = "testkit")]
pub mod testkit;
mod wrap;

pub use locate::{HELPER, locate};
pub use probe::{Platform, Probe, ProbeError, VERSION, probe};
pub use spec::Spec;
pub use wrap::{Sandboxed, argv};

/// 助手的退出码：它自己出了错（参数、规格写坏了，收紧失败）。照 `env`、`timeout` 的约定。
pub const EXIT_HELPER: u8 = 125;
/// 助手的退出码：找到了命令，执行不了。
pub const EXIT_CANNOT_RUN: u8 = 126;
/// 助手的退出码：找不到命令。
pub const EXIT_NOT_FOUND: u8 = 127;
