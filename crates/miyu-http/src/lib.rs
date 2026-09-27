//! HTTP 执行器（`docs/designs/05-内核接口.md` 第七节「HTTP 执行器」）：第 3 层，真的发请求。
//!
//! 驱动只管翻译（`miyu-drivers`），这里照它编码好的字节发请求，流式地读回来，边读边交给它的
//! 解码器，解出来的增量马上交出去。多久没收到新的字节就算断了；内核叫停就马上停。一次只发一回：
//! 重试和接着说是内核的事（施工 3-5 下）。现在有的：
//!
//! - [`Endpoint`]：发给谁：地址、key、供应商另配的头；打印出来 key 写成 `***`；
//! - [`client()`]：一个核心一个 HTTP 客户端，连接跨请求复用；
//! - [`send()`]：发一次请求，读到说完、出错或者被叫停；
//! - `testkit`（开关）：测试用的假服务器。

mod client;
mod endpoint;
mod send;
#[cfg(feature = "testkit")]
pub mod testkit;

pub use client::{Proxy, client};
pub use endpoint::Endpoint;
/// HTTP 客户端：[`client()`] 造的那一个，连接跨请求复用。上层照这个名字拿着它，不用直接依赖 reqwest。
pub use reqwest::Client;
pub use send::{Attempt, Outcome, Progress, send};
