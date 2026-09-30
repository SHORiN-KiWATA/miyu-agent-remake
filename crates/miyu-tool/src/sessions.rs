//! 列会话的端口（`docs/blueprint/cross-session.md` 第一条，`tools/sessions.md`，施工 C-3）：`sessions` 这件工具只拿它，不认识
//! 会话表。执行器照这个会话抄好它自己的编号、属主，经会话表列出同一个属主的主会话。
//!
//! 端口由下层定义、上层装（`00-设计理念.md` 第四节「依赖与接口的规矩」）：这里只定义形状，执行器（`miyu-session`）照
//! 每一次调用造一个，交给 [`crate::Call::sessions`]。只有本机的主会话有；测试里的假调用没有，`sessions` 照「没有别的会话」答。
//!
//! 认会话编号（[`find_session`]）也在这里：她写的整个编号或者后缀，在一批会话里对。读、发给别的会话（C-4、C-5）照它认。

use std::fmt;
use std::future::Future;
use std::pin::Pin;

use miyu_kernel::id::SessionId;
use miyu_kernel::time::Timestamp;

use crate::Stop;

/// 列会话的那件工具的名字（`cross-session.md` 第九条）：只有本机的主会话，造会话时工具面上有它。
pub const SESSIONS: &str = "sessions";

/// 认的时候至少几位：和短编号一样长（`kernel/ids.md`「会话的短编号」）。
const SHORTEST: usize = 8;

/// 列出这个会话的属主的别的主会话。
pub trait SessionsPort: Send + Sync {
    /// 这个会话自己的编号：列表第一行写它。
    fn this(&self) -> &SessionId;

    /// 属主和这个会话一样的主会话（`session.created` 不带 `parent`），删了的不算，不含这个会话自己，不排先后。每个会话读一遍
    /// 它的日志；读下一个之前看一眼 `stop`，举起来了就不往下读，交回已经读到的。
    fn list<'a>(&'a self, stop: &'a Stop) -> Listing<'a>;
}

/// 列会话的 future。列不出来（放会话的目录读不了、核心正在停）交回原因，英文的一句。
pub type Listing<'a> = Pin<Box<dyn Future<Output = Result<Vec<MainSession>, String>> + Send + 'a>>;

/// 列出来的一个主会话（`cross-session.md` 第一条第 2 款）：`session.list` 的那一项多出的三格也照它算。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MainSession {
    /// 会话编号。
    pub id: SessionId,
    /// 标题：照日志里的 `session.meta_changed` 算，没有的是空的。
    pub title: String,
    /// 工作目录：日志里最后一条带 `cwd` 的 `turn.started` 的，没有就照 `session.created` 的，都没有（很早以前的日志）是 `~`。
    pub cwd: String,
    /// 这时有回合在进行：它在会话表里、内核说不空闲（等人确认、等人回答的也算）。没载入的都是闲。
    pub busy: bool,
    /// 最近一次动静：日志最后一条事件的时刻；日志后面坏了的，照坏的那一段以前的。
    pub last_active: Timestamp,
}

/// 照她写的 `written` 在 `among` 里认会话（`cross-session.md`「对外的样子」会话的短编号、第三条第 1 款）：整个编号相同，或者
/// 写的是至少 8 位的小写十六进制、编号以它结尾。别的写法（大写、带空格、不到 8 位）一个都对不上。
pub fn find_session<'a>(written: &str, among: impl IntoIterator<Item = &'a SessionId>) -> Found {
    let suffix = written.len() >= SHORTEST
        && written
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    let mut matched = among.into_iter().filter(|id| {
        let id = id.as_str();
        id == written || (suffix && id.ends_with(written))
    });
    match (matched.next(), matched.next()) {
        (None, _) => Found::None,
        (Some(one), None) => Found::One(one.clone()),
        (Some(_), Some(_)) => Found::Many,
    }
}

/// 认出来的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Found {
    /// 正好一个。
    One(SessionId),
    /// 一个都没有。
    None,
    /// 不止一个：要写得更长。
    Many,
}

/// 端口不打出里面的东西。
impl fmt::Debug for dyn SessionsPort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SessionsPort")
    }
}

/// 两个端口比的是不是同一个：[`crate::Call`] 照格子比较时用。
impl PartialEq for dyn SessionsPort {
    fn eq(&self, other: &dyn SessionsPort) -> bool {
        std::ptr::addr_eq(self, other)
    }
}

impl Eq for dyn SessionsPort {}

#[cfg(test)]
mod tests;
