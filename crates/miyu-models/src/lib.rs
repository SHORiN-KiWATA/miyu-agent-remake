//! 模型这一块的纯逻辑（`docs/blueprint/models.md`，第 2 层，施工 8-6 起）：不碰文件和网络，时刻由调用的一方交进来。
//!
//! - [`settings`]：模型这一块的配置项，登记进核心的清单（供应商、模型手写的资料、用途、目录怎么更新）；
//! - [`mod@reference`]：三种写法：读、哪里能写哪几种，挡位、池解析到端点（施工 8-8）；
//! - [`profile`]：认得出的供应商的档案（资源目录的 `models/profiles.toml`，核心读好交进来）；
//! - [`provider`]：一家供应商照这一轮的配置、档案、目录合出来的样子，一个引用这一轮发给谁，没有模型时说什么；
//! - [`keys`]：一个会话钉在哪一个 key 上，候选的先后；
//! - [`pools`]：池的成员、怎么分、指针怎么走（施工 8-8）；
//! - [`catalog`]：models.dev 的目录（施工 8-7）；[`matching`]：四层对目录、规整、认原厂；
//! - [`facts`]：一个模型的资料，每一格的值和来源；[`observed`]：用出来的、供应商的列表；
//! - [`Knowledge`]：查资料时手头的几份。
//!
//! 用它的：会话的路由（`miyu-session` 的 `route.rs`）每次请求照它挑端点、查资料，核心起来时读档案、目录交给路由，
//! 协议的 `model.list` 照它列模型。

pub mod catalog;
pub mod facts;
pub mod keys;
mod knowledge;
pub mod matching;
pub mod observed;
pub mod pools;
pub mod profile;
pub mod provider;
pub mod reference;
pub mod settings;

pub use knowledge::Knowledge;

#[cfg(test)]
mod test_support;
