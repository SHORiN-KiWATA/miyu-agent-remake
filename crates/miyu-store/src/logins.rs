//! 登录令牌 `home/<账号>/logins.json`（`docs/blueprint/web-module.md`「怎么走」第一条第 6、7 条，施工 W-8）：只存令牌的
//! SHA-256、造的时刻、过期的时刻。令牌是 32 个随机字节，哈希够了，用不着 argon2（「起草时定的」第 5 条）。
//!
//! 每次加的时候删掉过期的；最多 64 行，多了删最早造的。

use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use miyu_kernel::time::Timestamp;

use crate::config_file::WriteError;
use crate::private_json;
pub use crate::private_json::BadFile;

/// 文件名：在账号的家目录里。
pub const FILE: &str = "logins.json";

/// 现在的写法。
const VERSION: u32 = 1;

/// 最多记几个。
const MOST: usize = 64;

/// 读好的一份。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Logins {
    /// 写法的版本：现在是 1。
    pub version: u32,
    /// 一个登录令牌一行，照加的先后。
    pub tokens: Vec<Login>,
}

impl Default for Logins {
    fn default() -> Logins {
        Logins {
            version: VERSION,
            tokens: Vec::new(),
        }
    }
}

/// 一个登录令牌。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Login {
    /// 什么时候造的。
    pub created: Timestamp,
    /// 什么时候过期。
    pub expires: Timestamp,
    /// 令牌的 SHA-256：`sha256:` 加 64 位小写十六进制（[`digest`]）。
    pub hash: String,
}

impl Logins {
    /// 加上 `login`：先删掉 `now` 时已经过期的；多过 64 个的删最早造的。
    pub fn add(&mut self, login: Login, now: Timestamp) {
        self.tokens.retain(|each| each.expires > now);
        self.tokens.push(login);
        while self.tokens.len() > MOST {
            let oldest = self
                .tokens
                .iter()
                .enumerate()
                .min_by_key(|(_, each)| each.created)
                .map(|(at, _)| at);
            if let Some(at) = oldest {
                self.tokens.remove(at);
            }
        }
    }

    /// 哈希是 `hash` 的令牌在 `now` 时认不认：有、没过期。
    pub fn valid(&self, hash: &str, now: Timestamp) -> bool {
        self.tokens
            .iter()
            .any(|each| each.hash == hash && each.expires > now)
    }

    /// 作废哈希是 `hash` 的那一个：有的交回真。
    pub fn remove(&mut self, hash: &str) -> bool {
        let before = self.tokens.len();
        self.tokens.retain(|each| each.hash != hash);
        self.tokens.len() < before
    }

    /// 全部作废：交回作废了几个。
    pub fn clear(&mut self) -> usize {
        std::mem::take(&mut self.tokens).len()
    }
}

/// 令牌 `token` 的哈希：`sha256:` 加 64 位小写十六进制。
pub fn digest(token: &str) -> String {
    let hash = Sha256::digest(token.as_bytes());
    let hex: String = hash.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("sha256:{hex}")
}

/// 读到的一份和它的版本（写回时交给 [`write()`]）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Read {
    /// 读好的。没有这个文件的是空的。
    pub logins: Logins,
    /// 整份字节的 SHA-256；没有这个文件的是空的。
    pub version: Option<String>,
}

/// 读 `path`。没有这个文件的是空的。
///
/// # Errors
///
/// 读不了、太大、不是 UTF-8；不是这个形状、版本不是 1。
pub fn read(path: &Path) -> Result<Read, BadFile> {
    let (logins, version): (Logins, _) = private_json::read(path)?;
    if logins.version != VERSION {
        return Err(BadFile::Shape(format!(
            "unknown version {}",
            logins.version
        )));
    }
    Ok(Read { logins, version })
}

/// 把 `logins` 整份写进 `path`，Unix 上 0600。`read` 是上一次读到的版本（那时还没有的是空的）。
///
/// # Errors
///
/// 这一瞬间有人手改了（[`WriteError::Changed`]）；写不进。都是什么都没变。
pub fn write(path: &Path, logins: &Logins, read: Option<&str>) -> Result<(), WriteError> {
    private_json::write(path, logins, read)
}

#[cfg(test)]
mod tests;
