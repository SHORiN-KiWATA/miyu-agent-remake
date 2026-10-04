//! 只给自己看的 JSON 小文件（施工 W-8）：`system/accounts.json`、`home/<账号>/logins.json`。读照配置文件的规矩（没有的是
//! 空的、1 MiB、UTF-8、版本），写照密钥文件的规矩（顺着链接、临时文件、同步、替换前再读一次，Unix 上一律 0600）。
//! 整份一行 JSON，末尾一个换行。

use std::fmt;
use std::path::Path;

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::config_file::{self, Mode, ReadError, WriteError};

/// 读不成：读不了，或者不是这个形状。
#[derive(Debug)]
pub enum BadFile {
    /// 读不了、太大、不是 UTF-8。
    Read(ReadError),
    /// 不是这个形状：带原话。
    Shape(String),
}

impl fmt::Display for BadFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BadFile::Read(error) => write!(f, "{error}"),
            BadFile::Shape(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for BadFile {}

/// 读 `path`：没有的是 `T::default()`、版本是空的；有的交回读好的和它的版本（写回时交给 [`write()`]）。
pub(crate) fn read<T: DeserializeOwned + Default>(
    path: &Path,
) -> Result<(T, Option<String>), BadFile> {
    let Some(text) = config_file::read(path).map_err(BadFile::Read)? else {
        return Ok((T::default(), None));
    };
    let value =
        serde_json::from_str(&text.text).map_err(|error| BadFile::Shape(error.to_string()))?;
    Ok((value, Some(text.version)))
}

/// 把 `value` 整份写进 `path`，Unix 上 0600。`read` 是上一次读到的版本（那时还没有的是空的）：对不上的不写。
pub(crate) fn write<T: Serialize>(
    path: &Path,
    value: &T,
    read: Option<&str>,
) -> Result<(), WriteError> {
    let mut bytes = serde_json::to_vec(value).map_err(|error| {
        WriteError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, error))
    })?;
    bytes.push(b'\n');
    config_file::write_with(path, &bytes, read, Mode::Private, &mut |_| Ok(()))
}
