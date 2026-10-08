//! 网页软件（`docs/blueprint/web-module.md`「怎么走」第九条、第十一条，施工 W-9）：一个单独的程序，是核心的一个头。
//!
//! - [`serve`]：只听 `127.0.0.1` 的一个端口，单实例、空闲退出；给页面文件，`/ws` 把浏览器的 WebSocket 一帧一条转成核心
//!   协议的一行一条，经本机传输连核心，不读本机令牌。给页面、核对 Host 和 Origin、`/ws` 照转这几样在共用的
//!   `miyu-webserve` 里（施工 O-16，`webserve.md`「搬家表」）。
//! - `media`：`/media`，带登录令牌换票据，照票据一块块给 blob、本机文件，能分段（施工 W-10）。
//! - [`open`]：`miyu web` 交来的参数：确保 `serve` 在跑，第一次、`--reset` 的照终端的样子出示本机令牌要一次性码，开浏览器
//!   （连核心、开浏览器的那两样在 `miyu-webserve`）。
//! - [`settings`]：`resources/web/web.json`：出厂的端口、空闲多久、内容安全策略、媒体类型的表。
//!
//! 不依赖核心：只用 `miyu-ipc`、`miyu-store`、`miyu-log`、`miyu-webserve`（分层照 `01-架构.md` 第九节第 5 层）。

mod media;
pub mod open;
pub mod serve;
pub mod settings;
mod texts;

/// 运行日志的目标。
const TARGET: &str = "miyu::web";
