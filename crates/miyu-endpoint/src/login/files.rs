//! 凭据文件（施工 W-8）：`system/accounts.json`、管理员的 `logins.json` 怎么读、改、写，系统日志记一条。调用的一方拿着
//! [`super::Identity`] 的锁，一件件改。
//!
//! 读不了、坏了的：当是空的，记一行 `WARN accounts not read`、`WARN logins not read`；写回时照现在磁盘上的字节的版本盖掉它。
//! 写不下的记一行 `WARN … not written`，回 `internal_error`。

use serde::Serialize;

use miyu_kernel::time::Timestamp;
use miyu_store::config_file::{self, WriteError};
use miyu_store::{accounts, logins};

use super::{TARGET, by, login_expires, random_hex};
use crate::Core;
use crate::refusal::Refusal;

/// 读 `system/accounts.json`：坏了的当空的。
pub(super) fn read_accounts(core: &Core) -> accounts::Read {
    let path = core.root.system().join(accounts::FILE);
    accounts::read(&path).unwrap_or_else(|error| {
        tracing::warn!(target: TARGET, error = %error, "accounts not read");
        accounts::Read {
            accounts: accounts::Accounts::default(),
            version: current(&path),
        }
    })
}

/// 读管理员的 `logins.json`：坏了的当空的。
pub(super) fn read_logins(core: &Core) -> logins::Read {
    let path = logins_path(core);
    logins::read(&path).unwrap_or_else(|error| {
        tracing::warn!(target: TARGET, error = %error, "logins not read");
        logins::Read {
            logins: logins::Logins::default(),
            version: current(&path),
        }
    })
}

/// 给管理员设用户名 `username`、密码哈希 `hashed`：交回是不是第一次设。
pub(super) fn put_account(
    core: &Core,
    username: &str,
    hashed: String,
    now: Timestamp,
) -> Result<bool, Refusal> {
    let read = read_accounts(core);
    let mut accounts = read.accounts;
    let first = accounts.of(core.admin.as_str()).is_none();
    accounts.put(accounts::Account {
        id: core.admin.as_str().to_string(),
        username: username.to_string(),
        password: hashed,
        changed: now,
    });
    let path = core.root.system().join(accounts::FILE);
    accounts::write(&path, &accounts, read.version.as_deref())
        .map_err(|error| not_written("accounts not written", &error))?;
    Ok(first)
}

/// 造一个登录令牌，记进 `logins.json`：交回令牌、它的哈希、过期的时刻。
pub(super) fn issue(core: &Core, now: Timestamp) -> Result<(String, String, Timestamp), Refusal> {
    let token = random_hex()?;
    let hash = logins::digest(&token);
    let expires = login_expires(now);
    let read = read_logins(core);
    let mut logins = read.logins;
    logins.add(
        logins::Login {
            created: now,
            expires,
            hash: hash.clone(),
        },
        now,
    );
    logins::write(&logins_path(core), &logins, read.version.as_deref())
        .map_err(|error| not_written("logins not written", &error))?;
    Ok((token, hash, expires))
}

/// 作废登录令牌：`one` 是那一个的哈希，空的是全部。交回作废了几个；一个都没有的不写文件。
pub(super) fn revoke(core: &Core, one: Option<&str>) -> Result<usize, Refusal> {
    let read = read_logins(core);
    let mut logins = read.logins;
    let count = match one {
        Some(hash) => usize::from(logins.remove(hash)),
        None => logins.clear(),
    };
    if count > 0 {
        logins::write(&logins_path(core), &logins, read.version.as_deref())
            .map_err(|error| not_written("logins not written", &error))?;
    }
    Ok(count)
}

/// `account.password_set` 的 `body`：设成的用户名、是不是第一次。
#[derive(Serialize)]
struct PasswordSet<'a> {
    username: &'a str,
    first: bool,
}

/// 系统日志记一条 `account.password_set`。写不进的记一行 `WARN`，不往上报。
pub(super) fn journal(core: &Core, username: &str, first: bool, at: Timestamp) {
    let path = core.config().places.system_journal.clone();
    let shown = format!("system/{}", miyu_store::journal::FILE);
    crate::config::journal::record(
        &path,
        &shown,
        at,
        by(core),
        None,
        "account.password_set",
        &PasswordSet { username, first },
    );
}

/// 管理员的 `logins.json` 在哪。
fn logins_path(core: &Core) -> std::path::PathBuf {
    core.root.account_dir(&core.admin).join(logins::FILE)
}

/// 磁盘上现在那份的版本：坏了的文件写回时照它盖。没有的、读不了的是空的。
fn current(path: &std::path::Path) -> Option<String> {
    std::fs::read(path)
        .ok()
        .map(|bytes| config_file::version(&bytes))
}

/// 写不下：记一行，回 `internal_error`。
fn not_written(what: &'static str, error: &WriteError) -> Refusal {
    tracing::warn!(target: TARGET, error = %error, "{what}");
    Refusal::INTERNAL
}
