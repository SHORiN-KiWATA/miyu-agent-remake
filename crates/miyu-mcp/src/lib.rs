//! MCP 的客户端（施工 X-1，`docs/blueprint/mcp.md`）：经一对读写管道（服务进程的标准输出、标准输入）说 MCP，一行一条
//! JSON-RPC 2.0。不认识 Miyu 的工具目录、不起进程：起进程、登记工具、换结果块是核心那一边的事。
//!
//! - [`Client::connect`]：认这个服务是哪个时代（`docs/blueprint/mcp.md`「怎么走」第一条）：先发 `server/discover`，答得上的
//!   是新时代（2026-07-28 起，每个请求自己带版本）；答别的错、5 秒没答的是旧时代，退回 `initialize` 握手。认出来的时代
//!   交给调的一方缓存，下次照它直接说。
//! - [`Client::tools`]：`tools/list`，翻页列全。
//! - [`Client::call`]：`tools/call`；这个 future 没等到回应就被丢掉（超时、叫停），发 `notifications/cancelled`。
//! - [`Client::changes`]：服务说工具变了（`notifications/tools/list_changed`）的次数。
//! - 服务反过来要东西（`sampling/createMessage`、`roots/list`、`elicitation/create`……）一律回「没有这个方法」，`ping`
//!   回空的。

mod client;
mod content;
mod error;
mod link;
#[cfg(any(test, feature = "testkit"))]
pub mod testkit;
mod wire;

pub use client::{Client, Era, Hello, Waits};
pub use content::{Annotations, Called, Content, Listed, Tool};
pub use error::Failed;

/// 新时代我们说的版本，挑的时候从新往旧排。
pub const MODERN: &[&str] = &["2026-07-28"];

/// 旧时代我们认得的版本，握手时报第一个。
pub const LEGACY: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

/// 运行日志的来源。
const TARGET: &str = "miyu::mcp";
