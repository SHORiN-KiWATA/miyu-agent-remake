//! 会话 actor（`docs/designs/02-内核.md` 第七节「会话 actor 怎么跑」，施工 3-7 中）：把内核的会话
//! 状态机接上磁盘。
//!
//! 一个会话一个异步任务。人的命令、执行器的回报都进它的收件箱，一条一条送进内核；内核交出的动作
//! 照 02 第四节「执行器怎么回动作」的表回。追加的事件写进会话日志、同步到磁盘了，才回应命令、
//! 才推给订阅的头（`07-存储.md` S4）。
//!
//! - [`create()`]：造一个会话，照人格存下策略快照；
//! - [`load()`]：从磁盘载入一个会话，照快照重建策略；
//! - [`Handle`]：发命令、订阅、有计划地停下；[`Backlog`]：订阅时要补发的那一截（施工 3-8 六补）；[`Shown`]：给头看的限额和
//!   会话接下来请求的模型（[`Next`]，施工 8-10）；
//! - [`Models`]、[`ModelPort`]：给会话造请求模型的端口，和端口本身。[`Routes`] 照配置挑供应商、钉 key，经驱动和
//!   HTTP 执行器请求（施工 3-7 下、8-6），测试里照剧本回；[`ModelData`]：核心一份的模型资料，[`refresh_list`]：拉供应商的
//!   模型列表（施工 8-7）；[`OneShot`]：模型调用口的一次性入口，和会话的路由共用一份底子，发一次、拿整段回答（[`Ask`]、
//!   [`Answer`]、[`Unanswered`]，施工 8-20）；
//! - [`new_id`]：新的会话编号；
//! - [`SessionPort`]：造子会话、给别的会话发命令的端口（施工 7-5），会话表实现、造会话和载入时交进来。
//! - [`Jobs`]：执行器的任务表，核心里一张：后台命令活过起它的那次调用（施工 7-3）；
//! - [`Configs`]、[`ConfigSource`]：会话从哪取配置，回合开始时冻结一份（[`TurnConfig`]），这一轮的请求都照它（施工 8-4），
//!   连同取 key 的办法（[`Turn`]，施工 8-6）。

mod actor;
mod agents;
mod backlog;
mod blocking;
mod clock;
mod config;
mod effects;
mod guard;
mod handle;
mod job_ids;
mod jobs;
mod kinds;
mod lines;
mod messages;
mod open;
mod peers;
mod pictures;
mod port;
mod report;
mod reread;
mod restore;
mod route;
mod sandbox;
mod sessions;
mod shown;
mod spawn;
mod store;
#[cfg(feature = "testkit")]
pub mod testkit;
mod tools;

pub use agents::job_in;
pub use backlog::Backlog;
pub use clock::new_id;
pub use config::{ConfigSource, Configs, Turn, TurnConfig, fixed, fixed_with};
pub use handle::{Ended, Handle, Pushed, Stopped, Subscription};
pub use jobs::{Jobs, Peek, Unreadable, peek};
pub use open::{Create, CreateError, Load, LoadError, create, load};
pub use port::{Cancel, ForSession, ModelPort, Models, Reports, Sight};
pub use route::{
    Answer, Ask, IDLE, LOCAL_WAIT, ModelData, Observed, OneShot, Probe, Probed, Routes, Running,
    STALE, Stage, Unanswered, find_local, probe, read_observed, refresh_list,
};
pub use sandbox::SandboxCache;
pub use shown::{Next, Shown};
pub use spawn::{Child, Lineage, NotWatched, Pending, SessionPort};

/// 运行日志的来源：`session`（`28-运行日志.md` 第二节）。
const TARGET: &str = "miyu::session";
