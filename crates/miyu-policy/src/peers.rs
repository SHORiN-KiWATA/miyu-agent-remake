//! 别的会话发来的话（施工 C-2，`docs/blueprint/cross-session.md`「对外的样子」「样子」）：防刷屏的数装进快照（`peers`），
//! 标签的两份原文装进快照（`core.peers`，`resources/core/peers/` 下）。
//!
//! 以前造的快照里没有数的，照出厂的数：防刷屏不能因为会话旧就不管。没有标签的，读成没有：那种话照人的话原样渲染。

use std::collections::BTreeMap;

use miyu_assemble::PeerTexts as Rendered;
use miyu_kernel::session::Peers;
use miyu_kernel::template::Template;
use serde::{Deserialize, Serialize};

use crate::snapshot::{BuildError, Snapshot};

/// 防刷屏的数（`cross-session.md`「对外的样子」的策略数据）：造会话时冻结在快照里。施工 C-6 在这里加它用的两个数。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerNumbers {
    /// 同一个发话方在一个窗口里最多几句。
    pub burst: u64,
    /// 限速、去重看多久以内的，单位秒。
    pub window: u64,
    /// 还没听到的别的会话的话最多几句。
    pub unread: u64,
}

/// 出厂的防刷屏的数（`cross-session.md`「起草时定的」第 11 条，数是估的，待 C-7 实测）：同一个发话方 10 分钟最多 5 句，
/// 10 分钟内一字不差的不收，没听到的最多 50 句（照 Claude Code）。配置那一步能改。
pub const PEERS: PeerNumbers = PeerNumbers {
    burst: 5,
    window: 600,
    unread: 50,
};

impl PeerNumbers {
    /// 交给内核的样子。
    pub(crate) fn kernel(self) -> Peers {
        let count = |n: u64| usize::try_from(n).unwrap_or(usize::MAX);
        Peers {
            burst: count(self.burst),
            window: self.window,
            unread: count(self.unread),
        }
    }
}

/// 别的会话发来的话的标签（`peers/` 下，文件名是下划线换成 `-` 的同名 `.txt`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerTexts {
    /// 标签：字段 `id`，发话的会话的短编号。
    pub message_open: String,
    /// 收尾。
    pub message_close: String,
}

impl PeerTexts {
    /// 交给组装器的样子：标签读成模板，拿 `id` 试换一次。
    ///
    /// # Errors
    ///
    /// 标签的模板坏了，或者要了 `id` 以外的字段。
    pub(crate) fn rendered(&self) -> Result<Rendered, BuildError> {
        let bad = |error| BuildError::Texts {
            which: "session message texts",
            error,
        };
        let open = Template::parse(&self.message_open).map_err(bad)?;
        open.render(&BTreeMap::from([("id", "")])).map_err(bad)?;
        Ok(Rendered {
            open,
            close: self.message_close.clone(),
        })
    }
}

impl Snapshot {
    /// 防刷屏的数：快照里的，以前造的没有照出厂的。
    pub(crate) fn peers(&self) -> Peers {
        self.peers.unwrap_or(PEERS).kernel()
    }
}

#[cfg(test)]
mod tests;
