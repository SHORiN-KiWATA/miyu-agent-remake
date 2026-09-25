//! 会话的事件（`docs/designs/03-事件模型.md` 第三节「会话与回合的事件怎么写」）。

use serde::{Deserialize, Serialize};

use crate::id::{AccountId, ContentHash, VenueId};
use crate::text_enum::text_enum;

/// `session.created`：会话创建。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionCreated {
    pub owner: AccountId,
    pub venue: VenueId,
    /// 策略快照（03 E5）。
    pub policy: ContentHash,
    /// 开始时的权限。
    pub permission: Permission,
}

/// `session.policy_changed`：换了策略快照，或者换了权限，也可以一起换。
/// 谁换的看事件的 `by`：人改的是配置，内核换的是目录变了。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyChanged {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy: Option<ContentHash>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission: Option<Permission>,
}

/// `session.meta_changed`：改了哪项写哪项。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetaChanged {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned: Option<bool>,
}

/// 权限：常用的那一级，加上叠在上面的只读开关（`11-权限与沙盒.md` 第二节）。两格都写。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Permission {
    pub level: Level,
    pub read_only: bool,
}

text_enum!(
    /// 常用的那一级。不认识的级别当什么，由用到它的地方决定：按最严的算。
    Level {
        /// 工作区：在沙盒里跑，要越过沙盒才问人。
        Workspace = "workspace",
        /// 完全放开：不进沙盒，只有管理员能用。
        Full = "full",
    }
);

#[cfg(test)]
mod tests;
