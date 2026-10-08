//! 工具的接口和工具目录（`docs/designs/05-内核接口.md` 第六节「工具的规格」、第八节「启用、注册与
//! 目录快照」，施工 4-1）。
//!
//! 工具是内核之外的软件（`10-自带软件.md` B1）：每件工具报出自己的规格，核心起来时登记进工具目录，
//! 登记完就冻结。造会话时，会话照目录把工具面存进策略快照（`03-事件模型.md` E5），以后一直照快照发。
//!
//! - [`Spec`]：一件工具的规格，第一批四格；
//! - [`Tool`]：一件工具：报规格，报一次调用要碰的路径（[`Target`]，施工 4-3 下），执行一次调用（[`Call`] 进、
//!   [`Done`] 出，施工 4-2）；
//! - [`Catalog`]：工具目录，登记时查重名、名字和参数格式的写法；
//! - [`Log`]：这个会话日志的只读入口（施工 6-4），`history` 用；
//! - [`AgentPort`]：派子代理的端口（施工 7-5），`subagent` 用；
//! - [`MessagePort`]：发话的端口（施工 7-7、C-5），`send_message` 用；
//! - [`JobPort`]：任务端口（施工 7-3），`shell` 把起好的后台命令交给它，`jobs` 经它查、停（施工 7-4）；
//! - [`SessionsPort`]：列会话、只读地开别的会话的日志（施工 C-3、C-4），`sessions`、`history` 用；[`find_session`]
//!   认她写的会话编号；
//! - [`UsagePort`]：查这个会话的用量、金额、上下文（施工 8-15），`session_usage` 用；
//! - [`picture`]：什么算一张图（施工 4-13 定，3-9 三补挪来）：`read` 读到的、人附上的，都照它认。

mod agents;
mod catalog;
mod jobs;
pub mod load;
mod log;
mod memory;
mod messages;
pub mod picture;
mod questions;
mod run;
mod sessions;
mod shelf;
mod stop;
#[cfg(feature = "testkit")]
pub mod testkit;
mod todos;
mod usage;

pub use agents::{
    AgentPort, NotSpawned, Order, SUBAGENT, SUBAGENT_FORMERLY, Spawned, Spawning, is_subagent,
};
pub use catalog::{BASESYSTEM, Catalog, CatalogError, Problem};
pub use jobs::{Asking, Background, Exit, JobError, JobPort, Listed, Output, Process};
pub use log::{Log, ReadLog};
pub use memory::{
    FORGET, FoundMemory, FoundTurn, MEMORIES, MEMORY_SEARCH, MemoryPort, Pending, REMEMBER,
    Refused, Remember, Searched, TEXT_CHARS, TURNS,
};
pub use messages::{
    Delivered, MessagePort, NotSent, Recipient, SEND_MESSAGE, SEND_MESSAGE_FORMERLY, Sending,
};
pub use questions::{ASK_USER, Answering, QuestionPort};
pub use run::{Call, CallIds, Done, Effect, Picture, Progress, Running, Seen, Target};
pub use sessions::{Found, Listing, MainSession, Opening, SESSIONS, SessionsPort, find_session};
pub use shelf::{Edition, Shelf};
pub use stop::Stop;
pub use todos::TODOWRITE;
pub use usage::{ContextUse, SESSION_USAGE, Spending, Spent, UsagePort};

use miyu_kernel::raw::RawJson;
use miyu_kernel::tool::Access;

/// 一件工具的规格（05 第六节）：第一批四格（施工 4-1）。显示名、摘要随施工 4-5；默认超时、场所、
/// 组，用得上时再加。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spec {
    /// 工具名：英文，稳定不变，模型照它调。只用英文字母、数字、`_`、`-`，1 到 64 个字符。
    pub name: String,
    /// 给模型看的说明，英文，原样进 tools 数组。
    pub description: String,
    /// 参数的 JSON Schema，原样进 tools 数组，一个字节不改：必须是 `{"type":"object",…}`。
    pub parameters: RawJson,
    /// 访问类别：权限策略、能不能和别的一起跑，都看它。
    pub access: Access,
}

/// 提供者的工具给哪种会话（施工 O-2 上，`docs/blueprint/providers.md`）：本机的、通讯平台的私聊、群。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Venues {
    /// 本机的会话。
    pub local: bool,
    /// 通讯平台的私聊。
    pub private: bool,
    /// 群。
    pub group: bool,
}

/// 一件工具。
pub trait Tool: Send + Sync {
    /// 它的规格。
    fn spec(&self) -> &Spec;

    /// 给哪种会话（施工 O-2 上）：提供者的工具照它挑。核心自带的没有，照工具面自己的规矩挑（`miyu-session` 的 `Agents::face`）。
    fn venues(&self) -> Option<Venues> {
        None
    }

    /// 这次调用要碰的路径、是读是写：照参数算，不碰磁盘（施工 4-3 下）。换成真实的位置、查边界是执行前的
    /// 链的事。默认一条都没有，例如执行命令。参数不对的，也交回空的：执行时再报错。
    fn targets(&self, _call: &Call) -> Vec<Target> {
        Vec::new()
    }

    /// 这次调用要在沙盒外跑（施工 D-4）：照参数算，不碰磁盘。权限策略照它判（工作区问人、只读拒绝），执行器照它不写
    /// 沙盒的规格：工具自己从不决定套不套沙盒。默认否。
    fn outside_sandbox(&self, _call: &Call) -> bool {
        false
    }

    /// 问人时交给说明（`detail`）的几格，键和值（施工 D-4）：照参数算，例如 `shell` 的 `title`、`command`。权限策略并进
    /// 它自己写的那几格。参数不对的交回空的。默认空。
    fn asking(&self, _call: &Call) -> Vec<(&'static str, String)> {
        Vec::new()
    }

    /// 执行一次调用：执行器在它自己的任务里跑交回的 future，执行中的输出交给 `progress`。叫停有两种：
    /// 「叫它停」举 [`Call::stop`] 的旗，等它交回来（施工 4-9 再补一）；「掐掉」丢掉这个 future（施工 4-2）。
    fn run(&self, call: Call, progress: Progress) -> Running<'_>;

    /// 它以前的名字（施工 7-5 再补）：改过名的工具，改名以前造的会话快照里冻着旧名字（工具面是前缀，不能变），她照旧名字调，
    /// 工具目录照旧名字也找得到这一件（[`Catalog::get`]）。旧名字不进 [`Catalog::specs`]：新造的会话工具面上只有现在的名字。
    /// 默认没有。
    fn formerly(&self) -> &'static [&'static str] {
        &[]
    }
}
