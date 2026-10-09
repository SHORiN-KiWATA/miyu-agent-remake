//! 快照的两种错（从 `snapshot.rs` 挪出来，那边放不下了）：字节读不回来，照快照造不出策略。

use std::fmt;

use miyu_kernel::template::TemplateError;

/// 快照的字节读不回来：不是这个版本写的，或者坏了。还没发布，格式改了不背兼容。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotError(pub(super) String);

impl fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "policy snapshot not readable: {}", self.0)
    }
}

impl std::error::Error for SnapshotError {}

/// 照快照造不出策略。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildError {
    /// 随核心附带的哪一份字用不了。
    Texts {
        /// 哪一类：事实的模板、内核替工具写的几句、驱动的占位。
        which: &'static str,
        /// 哪里坏了。
        error: TemplateError,
    },
    /// 工具面上有两件叫这个名字的（施工 4-1）：她调的是哪一件，说不清。
    DuplicateTool(String),
    /// 群会话钉下的时区超出了 −14:00 到 +14:00（施工 O-13 中）：快照坏了。
    Offset(i32),
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BuildError::Texts { which, error } => write!(f, "bundled {which} not usable: {error}"),
            BuildError::DuplicateTool(name) => write!(f, "two tools named {name:?}"),
            BuildError::Offset(minutes) => {
                write!(f, "group chat time zone out of range: {minutes} minutes")
            }
        }
    }
}

impl std::error::Error for BuildError {}
