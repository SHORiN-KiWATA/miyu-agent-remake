//! 读、写密钥文件 `system/secrets.toml`（`docs/blueprint/config.md`「怎么走」第九条第 1、2、4 条，G9，施工 8-5）。
//!
//! 读照配置文件的规矩（[`config_file::read`]：没有的是空的、1 MiB、BOM、UTF-8、版本），另外看一眼权限：Unix 上组、别人
//! 读得到的照用，交回 [`Stored::open`] 由调用的一方记 `WARN secrets readable by others`，不去改它（`miyu doctor` 以后报）。
//!
//! 写照配置文件的规矩（顺着链接、临时文件、同步、替换前再读一次、Windows 上重试），只是 Unix 上一律 0600：临时文件建的
//! 时候就是 0600，不带原文件的权限位，写一次就把手改松了的权限收回来。Windows 上照数据根继承的访问控制。
//!
//! 这里只管字节。字里的密钥怎么读、怎么改一行，在 `miyu-config` 的 `secret.rs`。

use std::path::Path;

use crate::config_file::{self, ConfigText, Mode, ReadError, WriteError};

/// 文件名：在 `system/` 里。
pub const FILE: &str = "secrets.toml";

/// 读好的密钥文件。
#[derive(Clone, PartialEq, Eq)]
pub struct Stored {
    /// 字和版本，照配置文件的读法。
    pub text: ConfigText,
    /// Unix 上组或者别人读得到（权限位里有 `0o044` 的任何一位）。Windows 上恒为 `false`。
    pub open: bool,
}

/// 字里是密钥：`Debug` 不印字。
impl std::fmt::Debug for Stored {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Stored")
            .field("version", &self.text.version)
            .field("open", &self.open)
            .finish_non_exhaustive()
    }
}

/// 读 `path` 这一份密钥文件。没有这个文件的是空的。
///
/// # Errors
///
/// 读不了、太大、不是 UTF-8。
pub fn read(path: &Path) -> Result<Option<Stored>, ReadError> {
    let Some(text) = config_file::read(path)? else {
        return Ok(None);
    };
    Ok(Some(Stored {
        text,
        open: open_to_others(path),
    }))
}

/// 把 `path` 这一份密钥文件整份写成 `content`，Unix 上是 0600。`read` 是上一次读到的版本（那时还没有的是空的）。
///
/// # Errors
///
/// 同 [`config_file::write`]：这一瞬间有人手改了（[`WriteError::Changed`]），写不成。都是什么都没变。
pub fn write(path: &Path, content: &[u8], read: Option<&str>) -> Result<(), WriteError> {
    config_file::write_with(path, content, read, Mode::Private, &mut |_| Ok(()))
}

/// 组、别人读不读得到（顺着链接看本体）。看不了的当读不到：读得进来的文件，看不了权限是少见的事，不为它报一条假的。
#[cfg(unix)]
fn open_to_others(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|metadata| metadata.permissions().mode() & 0o044 != 0)
}

/// Windows 上照数据根继承的访问控制，不另看。
#[cfg(not(unix))]
fn open_to_others(_path: &Path) -> bool {
    false
}

#[cfg(test)]
mod tests;
