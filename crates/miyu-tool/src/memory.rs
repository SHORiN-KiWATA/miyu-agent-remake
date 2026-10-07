//! 交给记忆的三件工具的端口（施工 R-3 中，`docs/blueprint/memory.md`「工具」）：记一条、作废一条、搜记下的和以前的对话。
//!
//! 工具只拿这个端口，不认识记忆日志、检索库、会话：执行器每次调用照这个会话、这一轮、属主、人格、听众造一个（实现在
//! `miyu-session`），碰磁盘的在阻塞线程里做。工具照它交回的写给她看的那几句。

use std::future::Future;
use std::pin::Pin;

use miyu_kernel::id::{SessionId, TurnId};
use miyu_kernel::time::Timestamp;
use miyu_recall::MemoryId;

/// 记一条的那件工具的名字：只有记忆开着的本机主会话，造会话时工具面上有它（施工 R-3 中）。
pub const REMEMBER: &str = "remember";
/// 作废一条的那件工具的名字。
pub const FORGET: &str = "forget";
/// 搜记忆的那件工具的名字。
pub const MEMORY_SEARCH: &str = "memory_search";

/// `memory_search` 最多给几条记下的：端口照它挑（出厂值只在这一处，以后要改再进配置）。
pub const MEMORIES: usize = 10;

/// `memory_search` 最多给几段以前的对话。
pub const TURNS: usize = 5;

/// 记忆的端口。
pub trait MemoryPort: Send + Sync {
    /// 记一条：出处是这一轮，听众是这一轮的，`by` 是这次调用；写了 `replaces` 的是改那一条。交回它的编号。
    fn save<'a>(&'a self, remember: Remember) -> Pending<'a, Result<MemoryId, Refused>>;

    /// 作废一条。
    fn retire<'a>(&'a self, id: MemoryId, why: String) -> Pending<'a, Result<(), Refused>>;

    /// 搜记下的和以前的对话。`forgotten` 时连作废的一起。只交现在算数的、出处还活着的、听众合的，最多 [`MEMORIES`] 条记下的、
    /// [`TURNS`] 段以前的对话（这个会话自己的不算），最相关的在前。
    fn search<'a>(
        &'a self,
        query: String,
        forgotten: bool,
    ) -> Pending<'a, Result<Searched, String>>;
}

/// 端口交回的 future。
pub type Pending<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// 记一条要的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Remember {
    /// 类：四类之一，工具查过。
    pub class: String,
    /// 正文：工具查过长度。
    pub text: String,
    /// 改的是哪一条。
    pub replaces: Option<MemoryId>,
}

/// 记不下、作废不了。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    /// 没有这一条（听众不合的也当没有：不让她知道有）。
    NoSuch(MemoryId),
    /// 这一条已经被改掉、作废了。
    NotCurrent(MemoryId),
    /// 写不进、读不了：为什么。
    Failed(String),
}

/// 搜到的。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Searched {
    /// 记下的，最相关的在前。
    pub memories: Vec<FoundMemory>,
    /// 以前的对话，最相关的在前。
    pub turns: Vec<FoundTurn>,
}

/// 搜到的一条记下的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundMemory {
    /// 编号。
    pub id: MemoryId,
    /// 类。
    pub class: String,
    /// 正文。
    pub text: String,
    /// 记下的时刻。
    pub at: Timestamp,
    /// 作废了（`forgotten` 时才会有）。
    pub retired: bool,
}

/// 搜到的一轮以前的对话。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundTurn {
    /// 哪个会话。
    pub session: SessionId,
    /// 哪一轮。
    pub turn: TurnId,
    /// 那一轮的时刻。
    pub at: Timestamp,
    /// 人的话加她的回答（回合索引里的一条）。
    pub text: String,
}

impl std::fmt::Debug for dyn MemoryPort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MemoryPort")
    }
}

/// 两个端口比的是不是同一个：[`crate::Call`] 照格子比较时用。
impl PartialEq for dyn MemoryPort {
    fn eq(&self, other: &dyn MemoryPort) -> bool {
        std::ptr::addr_eq(self, other)
    }
}

impl Eq for dyn MemoryPort {}
