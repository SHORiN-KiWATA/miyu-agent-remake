//! 会话 actor（`docs/designs/02-内核.md` 第七节「会话 actor 怎么跑」，施工 3-7 中）：把内核的会话
//! 状态机接上磁盘。
//!
//! 一个会话一个异步任务。人的命令、执行器的回报都进它的收件箱，一条一条送进内核；内核交出的动作
//! 照 02 第四节「执行器怎么回动作」的表回。追加的事件写进会话日志、同步到磁盘了，才回应命令、
//! 才推给订阅的头（`07-存储.md` S4）。
//!
//! - [`create()`]：造一个会话，照人格存下策略快照；
//! - [`load()`]：从磁盘载入一个会话，照快照重建策略；
//! - [`Handle`]：发命令、订阅、有计划地停下；
//! - [`Models`]、[`ModelPort`]：给会话造请求模型的端口，和端口本身。[`HttpModels`] 经驱动和 HTTP
//!   执行器请求（施工 3-7 下），测试里照剧本回；
//! - [`new_id`]：新的会话编号；
//! - [`SessionPort`]：造子会话、给别的会话发命令的端口（施工 7-5），会话表实现、造会话和载入时交进来。
//! - [`Jobs`]：执行器的任务表，核心里一张：后台命令活过起它的那次调用（施工 7-3）。

mod actor;
mod agents;
mod blocking;
mod clock;
mod effects;
mod guard;
mod handle;
mod http;
mod job_ids;
mod jobs;
mod kinds;
mod lines;
mod open;
mod pictures;
mod port;
mod report;
mod reread;
mod restore;
mod sandbox;
mod spawn;
mod store;
#[cfg(feature = "testkit")]
pub mod testkit;
mod tools;

pub use clock::new_id;
pub use handle::{Ended, Handle, Pushed, Stopped, Subscription};
pub use http::{HttpModels, IDLE};
pub use jobs::Jobs;
pub use open::{Create, CreateError, Load, LoadError, create, load};
pub use port::{Cancel, ForSession, ModelPort, Models, Reports};
pub use sandbox::SandboxCache;
pub use spawn::{Child, Lineage, Pending, SessionPort};

/// 运行日志的来源：`session`（`28-运行日志.md` 第二节）。
const TARGET: &str = "miyu::session";
