//! 别的会话发来的话（施工 C-2，`docs/blueprint/cross-session.md`「对外的样子」「样子」）：防刷屏的数装进快照（`peers`），
//! 标签的两份原文装进快照（`core.peers`，`resources/core/peers/` 下）。
//!
//! 以前造的快照里没有数的，照出厂的数：防刷屏不能因为会话旧就不管。没有标签的，读成没有：那种话照人的话原样渲染。
//!
//! 「空了告诉我」（施工 C-6）在同一处加两个数（`watch_hours`、`status_chars`）、通知的五份字（`idle-*.txt`）。C-2 时造的快照
//! 里没有：数照出厂的，字读成没有（那种会话订不了，也就没有通知要渲染）；读进来再写出去一字不差。

use std::collections::BTreeMap;

use miyu_assemble::{IdleTexts as RenderedIdle, PeerTexts as Rendered};
use miyu_kernel::session::Peers;
use miyu_kernel::template::Template;
use serde::{Deserialize, Serialize};

use crate::snapshot::{BuildError, Snapshot};

/// 防刷屏的数（`cross-session.md`「对外的样子」的策略数据）：造会话时冻结在快照里。施工 C-6 加了它用的两个数。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerNumbers {
    /// 同一个发话方在一个窗口里最多几句。
    pub burst: u64,
    /// 限速、去重看多久以内的，单位秒。
    pub window: u64,
    /// 还没听到的别的会话的话最多几句。
    pub unread: u64,
    /// 订了多久没等到通知就作废，单位小时（施工 C-6）。C-2 时造的快照里没有，照出厂的；没有的不写。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub watch_hours: Option<u64>,
    /// 通知里带的那一行最多几个字（施工 C-6）。C-2 时造的快照里没有，照出厂的；没有的不写。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_chars: Option<u64>,
}

/// 订了多久作废的出厂值，单位小时（`cross-session.md`「起草时定的」第 10 条，照 Claude Code）。
const WATCH_HOURS: u64 = 12;

/// 通知那一行最多几个字的出厂值（「起草时定的」第 9 条）。
const STATUS_CHARS: u64 = 200;

/// 出厂的数（`cross-session.md`「起草时定的」第 11 条，数是估的，待 C-7 实测）：同一个发话方 10 分钟最多 5 句，
/// 10 分钟内一字不差的不收，没听到的最多 50 句（照 Claude Code）；订了 12 小时作废，通知那一行最多 200 个字（施工 C-6）。
/// 配置那一步能改。
pub const PEERS: PeerNumbers = PeerNumbers {
    burst: 5,
    window: 600,
    unread: 50,
    watch_hours: Some(WATCH_HOURS),
    status_chars: Some(STATUS_CHARS),
};

impl PeerNumbers {
    /// 交给内核的样子。
    pub(crate) fn kernel(self) -> Peers {
        let count = |n: u64| usize::try_from(n).unwrap_or(usize::MAX);
        Peers {
            burst: count(self.burst),
            window: self.window,
            unread: count(self.unread),
            watch_hours: self.watch_hours(),
            status_chars: count(self.status_chars.unwrap_or(STATUS_CHARS)),
        }
    }

    /// 订了多久作废：快照里的，C-2 时造的没有照出厂的。
    fn watch_hours(self) -> u64 {
        self.watch_hours.unwrap_or(WATCH_HOURS)
    }
}

/// 别的会话发来的话的标签（`peers/` 下，文件名是下划线换成 `-` 的同名 `.txt`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerTexts {
    /// 标签：字段 `id`，发话的会话的短编号。
    pub message_open: String,
    /// 收尾。
    pub message_close: String,
    /// 空了的通知（施工 C-6）：和标签平铺在同一层，文件名照同一个规矩。C-2 时造的快照里没有，读成没有、不写。
    #[serde(flatten, default, skip_serializing_if = "Option::is_none")]
    pub idle: Option<PeerIdleTexts>,
}

/// 空了的通知的五份字（`peers/idle-*.txt`，施工 C-6）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerIdleTexts {
    /// 标签：字段 `id` 是等的那个会话的短编号，`reason` 是原因。
    pub idle_open: String,
    /// 那一轮一个字都没说。
    pub idle_silent: String,
    /// 作废了：字段 `hours`，照快照里的 `peers.watch_hours` 换。
    pub idle_expired: String,
    /// 那个会话不在了。
    pub idle_gone: String,
    /// 收尾。
    pub idle_close: String,
}

impl PeerTexts {
    /// 交给组装器的样子：标签读成模板，拿 `id` 试换一次；通知的标签拿 `id`、`reason` 试换，作废那一句照 `hours` 换好。
    ///
    /// # Errors
    ///
    /// 标签、通知的模板坏了，或者要了别的字段。
    pub(crate) fn rendered(&self, hours: u64) -> Result<Rendered, BuildError> {
        let bad = |error| BuildError::Texts {
            which: "session message texts",
            error,
        };
        let open = Template::parse(&self.message_open).map_err(bad)?;
        open.render(&BTreeMap::from([("id", "")])).map_err(bad)?;
        Ok(Rendered {
            open,
            close: self.message_close.clone(),
            idle: self
                .idle
                .as_ref()
                .map(|idle| idle.rendered(hours))
                .transpose()?,
        })
    }
}

impl PeerIdleTexts {
    /// 交给组装器的样子。
    fn rendered(&self, hours: u64) -> Result<RenderedIdle, BuildError> {
        let bad = |error| BuildError::Texts {
            which: "session idle texts",
            error,
        };
        let open = Template::parse(&self.idle_open).map_err(bad)?;
        open.render(&BTreeMap::from([("id", ""), ("reason", "")]))
            .map_err(bad)?;
        let hours = hours.to_string();
        let expired = Template::parse(&self.idle_expired)
            .and_then(|expired| expired.render(&BTreeMap::from([("hours", hours.as_str())])))
            .map_err(bad)?;
        Ok(RenderedIdle {
            open,
            silent: self.idle_silent.clone(),
            expired,
            gone: self.idle_gone.clone(),
            close: self.idle_close.clone(),
        })
    }
}

impl Snapshot {
    /// 防刷屏的数：快照里的，以前造的没有照出厂的。
    pub(crate) fn peers(&self) -> Peers {
        self.peers.unwrap_or(PEERS).kernel()
    }

    /// 别的会话发来的话、空了的通知的字（施工 C-2、C-6）：作废那一句照这份快照的 `watch_hours` 换好。
    pub(crate) fn peer_texts(&self) -> Result<Option<Rendered>, BuildError> {
        let hours = self.peers.unwrap_or(PEERS).watch_hours();
        self.core
            .peers
            .as_ref()
            .map(|texts| texts.rendered(hours))
            .transpose()
    }
}

#[cfg(test)]
mod tests;
