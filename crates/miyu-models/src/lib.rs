//! 模型这一块的纯逻辑（`docs/blueprint/models.md`，第 2 层，施工 8-6 起）：不碰文件和网络，时刻由调用的一方交进来。
//!
//! - [`settings`]：模型这一块的配置项，登记进核心的清单（供应商、模型手写的资料、用途）；
//! - [`mod@reference`]：三种写法：读、哪里能写哪几种；
//! - [`profile`]：认得出的供应商的档案（资源目录的 `models/profiles.toml`，核心读好交进来）；
//! - [`provider`]：一家供应商照这一轮的配置和档案合出来的样子，一个引用这一轮发给谁，没有模型时说什么；
//! - [`keys`]：一个会话钉在哪一个 key 上，候选的先后；
//! - [`ModelTable`]：模型资料（窗口、最大输出），资源目录的 `models/models-dev.json`。8-7 换成完整的目录。
//!
//! 用它的：会话的路由（`miyu-session` 的 `route.rs`）每次请求照它挑端点，核心起来时读档案、模型资料交给路由。

pub mod keys;
pub mod profile;
pub mod provider;
pub mod reference;
pub mod settings;
mod table;

pub use table::{ModelFacts, ModelTable};
