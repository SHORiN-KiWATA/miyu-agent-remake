//! 给界面用的门面（设计 `32-仓库拆分.md` 第二节，施工 S-1）：终端、网页、接入QQ 这些头连核心要的几样都从这里拿，不再直接
//! 依赖核心里别的 crate。代码大多留在原来的 crate，这里 `pub use` 转出去；门面上有的就是界面能用的，改了、少了照改协议的
//! 规矩告诉两个头（`tests.rs` 把每一样用一遍钉住）。
//!
//! - [`connect`]：经本机传输连核心、没在跑时拉起来（`miyu-ipc`）。
//! - [`open`]：照终端的样子连核心（出示本机令牌）、一问一答，带一次性码开浏览器（从 `miyu-webserve` 挪来）。
//! - [`places`]：数据根、资源目录、环境、语言、缓存目录在哪（`miyu-store`）。
//! - [`texts`]：给人看的字（`miyu-store` 的 `human`）。
//! - [`protocol`]：协议里的类型：编号、事件、模板（`miyu-kernel`）。
//! - [`manifest`]：读自己的软件包清单（`miyu-config`）。
//! - [`log`]：运行日志（`miyu-log`）。

pub mod open;
#[cfg(test)]
mod tests;

use std::process::Command;
use std::sync::Arc;

/// 拉起核心的命令：主程序 `miyu` 加 `core`。
pub type CoreCommand = Arc<dyn Fn() -> Command + Send + Sync>;

/// 照钉住的那份核心检出的根目录（仓库根）：界面的测试照它读 `resources/`、`docs/designs/samples/`（设计 32 第四节第 3 条）。
/// git 依赖在 cargo 的检出里是整个仓库，资源和样本跟着钉住的提交走。只在 `testkit` 打开时有。
#[cfg(feature = "testkit")]
pub const WORKSPACE_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

/// 经本机传输连核心、没在跑时拉起来。
pub mod connect {
    pub use miyu_ipc::{
        ConnectError, Connection, Ready, StartError, connect, connect_or_start,
        connect_or_start_bare, spawn_detached,
    };
}

/// 数据根、资源目录、环境、语言、缓存目录在哪。
pub mod places {
    pub use miyu_store::env::{Env, Platform, locale};
    pub use miyu_store::resources::ResourceRoot;
    pub use miyu_store::root::{DataRoot, cache_root};
}

/// 给人看的字：照语言读出来、画工具的显示名和说法。
pub mod texts {
    pub use miyu_store::human::{Face, Human, clean};
}

/// 协议里的类型。
pub mod protocol {
    pub use miyu_kernel::event::Said;
    pub use miyu_kernel::id::SessionId;
    pub use miyu_kernel::template::Template;
}

/// 读自己的软件包清单、清单里的值。
pub mod manifest {
    pub use miyu_config::Value;
    pub use miyu_config::package::read;
}

/// 运行日志。
pub mod log {
    pub use miyu_log::{LevelFilter, install};
}
