//! 读配置文件（`docs/blueprint/config.md`「怎么走」第二条第 2 条，施工 8-2）：系统配置、个人设置、项目配置、信任的记录
//! 都照它读。写的那一半随 8-3。
//!
//! 1. 没有这个文件：这一层是空的，不算问题（[`read`] 交回 `Ok(None)`）。
//! 2. 顺着链接找到本体再读（系统打开文件时本来就顺着链接）；读不了：[`ReadError::Unreadable`]。
//! 3. 超过 1 MiB：[`ReadError::TooBig`]，读到上限多一个字节就停，不往内存里读更多。
//! 4. 开头的 UTF-8 BOM 去掉；不是 UTF-8：[`ReadError::NotUtf8`]。
//! 5. 版本是整份字节（带 BOM）的 SHA-256，写成 `sha256:` 加 64 位十六进制。

use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use sha2::{Digest, Sha256};

/// 一份配置文件最多多大：1 MiB。
pub const LIMIT: u64 = 1024 * 1024;

/// 开头的 UTF-8 BOM。
const BOM: &str = "\u{FEFF}";

/// 读好的一份。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigText {
    /// 字，开头的 BOM 去掉了。
    pub text: String,
    /// 版本：整份字节的 SHA-256，`sha256:` 开头。
    pub version: String,
}

/// 读不成一份配置文件。
#[derive(Debug)]
pub enum ReadError {
    /// 读不了：没有权限、是个目录……带系统的原话。
    Unreadable(io::Error),
    /// 超过 1 MiB。
    TooBig,
    /// 不是 UTF-8。
    NotUtf8,
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReadError::Unreadable(error) => write!(f, "{error}"),
            ReadError::TooBig => write!(f, "over {LIMIT} bytes"),
            ReadError::NotUtf8 => write!(f, "not UTF-8"),
        }
    }
}

impl std::error::Error for ReadError {}

/// 读 `path` 这一份配置文件。没有这个文件的是空的。
///
/// # Errors
///
/// 读不了、太大、不是 UTF-8。
pub fn read(path: &Path) -> Result<Option<ConfigText>, ReadError> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(ReadError::Unreadable(error)),
    };
    let mut bytes = Vec::new();
    file.take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(ReadError::Unreadable)?;
    if bytes.len() as u64 > LIMIT {
        return Err(ReadError::TooBig);
    }
    Ok(Some(text(&bytes)?))
}

/// 一份文件的字节变成字和版本：去掉开头的 BOM，不是 UTF-8 的报错。
///
/// # Errors
///
/// 不是 UTF-8。
pub fn text(bytes: &[u8]) -> Result<ConfigText, ReadError> {
    let text = std::str::from_utf8(bytes).map_err(|_| ReadError::NotUtf8)?;
    Ok(ConfigText {
        text: text.strip_prefix(BOM).unwrap_or(text).to_string(),
        version: version(bytes),
    })
}

/// 版本：`sha256:` 加整份字节的 SHA-256，64 位小写十六进制。
pub fn version(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("sha256:{hex}")
}

#[cfg(test)]
mod tests;
