//! 一处记忆（「房间」，施工 R-3 下，`docs/blueprint/memory.md`「范围」）：一个范围的记忆日志、回合库、记忆库放在哪。
//!
//! - 跟着人格的：记忆归哪个账号（`memory_account`，P-1 上）的家目录下，`modules/memory/<人格>/`、`index/recall/turns-<人格>.db`、
//!   `index/recall/memory-<人格>.db`。
//! - 只在这个会话里的：会话自己的目录 `sessions/<会话>/memory/` 下，`log/`、`turns.db`、`memory.db`。删会话、进回收处、恢复
//!   都跟着目录走，不另写一行。

use std::fmt;
use std::path::PathBuf;

use miyu_kernel::id::{AccountId, SessionId};

use crate::root::DataRoot;

/// 回合库文件名的前后：`turns-<人格>.db`。
pub(crate) const TURNS_PREFIX: &str = "turns-";
/// 库文件的后缀。
pub(crate) const DB: &str = ".db";

/// 一处记忆。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Room {
    /// 跟着人格：这个账号、这个人格的。
    Persona {
        /// 记忆归哪个账号。
        account: AccountId,
        /// 人格的编号。
        persona: String,
    },
    /// 只在这个会话里：会话的属主、会话编号。
    Session {
        /// 会话的属主。
        account: AccountId,
        /// 会话编号。
        session: SessionId,
    },
}

/// 写进运行日志的样子：`persona <账号>/<人格>`、`session <账号>/<会话>`。
impl fmt::Display for Room {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Room::Persona { account, persona } => write!(f, "persona {account}/{persona}"),
            Room::Session { account, session } => write!(f, "session {account}/{session}"),
        }
    }
}

impl Room {
    /// 账号 `account`、人格 `persona` 的那一间。
    pub fn persona(account: &AccountId, persona: &str) -> Room {
        Room::Persona {
            account: account.clone(),
            persona: persona.to_string(),
        }
    }

    /// 属主 `account` 的会话 `session` 自己的那一间。
    pub fn session(account: &AccountId, session: &SessionId) -> Room {
        Room::Session {
            account: account.clone(),
            session: session.clone(),
        }
    }

    /// 回合库在哪。
    pub(crate) fn turns(&self, root: &DataRoot) -> PathBuf {
        match self {
            Room::Persona { account, persona } => {
                recall_dir(root, account).join(format!("{TURNS_PREFIX}{persona}{DB}"))
            }
            Room::Session { .. } => self.session_dir(root).join(format!("turns{DB}")),
        }
    }

    /// 记忆库在哪。
    pub(crate) fn memories(&self, root: &DataRoot) -> PathBuf {
        match self {
            Room::Persona { account, persona } => {
                recall_dir(root, account).join(format!("memory-{persona}{DB}"))
            }
            Room::Session { .. } => self.session_dir(root).join(format!("memory{DB}")),
        }
    }

    /// 记忆日志在哪。
    pub(crate) fn log(&self, root: &DataRoot) -> PathBuf {
        match self {
            Room::Persona { account, persona } => root
                .account_dir(account)
                .join("modules")
                .join("memory")
                .join(persona),
            Room::Session { .. } => self.session_dir(root).join("log"),
        }
    }

    /// 会话那一间的目录。
    fn session_dir(&self, root: &DataRoot) -> PathBuf {
        match self {
            Room::Session { account, session } => root.session_dir(account, session).join("memory"),
            Room::Persona { account, .. } => root.account_dir(account),
        }
    }
}

/// 一个账号放跟着人格的那几间检索库的目录。
pub(crate) fn recall_dir(root: &DataRoot, account: &AccountId) -> PathBuf {
    root.index(account).join("recall")
}
