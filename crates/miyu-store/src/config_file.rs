//! 读、写配置文件（`docs/blueprint/config.md`「怎么走」第二条第 2 条、第五条第 4 到 7 条）：系统配置、个人设置、项目配置、
//! 信任的记录都照它读、写。读的一半施工 8-2，写的一半施工 8-3。
//!
//! 读（[`read`]）：
//!
//! 1. 没有这个文件：这一层是空的，不算问题（交回 `Ok(None)`）。
//! 2. 顺着链接找到本体再读（系统打开文件时本来就顺着链接）；读不了：[`ReadError::Unreadable`]。
//! 3. 超过 1 MiB：[`ReadError::TooBig`]，读到上限多一个字节就停，不往内存里读更多。
//! 4. 开头的 UTF-8 BOM 去掉，记下有没有（写回时照样加回）；不是 UTF-8：[`ReadError::NotUtf8`]。
//! 5. 版本是整份字节（带 BOM）的 SHA-256，写成 `sha256:` 加 64 位十六进制。
//!
//! 写（[`write()`]）：
//!
//! 1. 顺着链接写：一层层找到它指向的本体（相对的照链接所在的目录接，最多 40 层，绕圈的报错），写本体，链接本身不动。
//!    指向的地方还没有文件的，在那里新建。
//! 2. 先写临时文件再替换：临时文件建在本体旁边，`.<文件名>.<进程号>-<计数>.tmp`，只许新建；写完、同步，Unix 上带上原文件
//!    的权限位（新文件照系统默认），再改名盖上本体，再同步目录。写到一半断电，磁盘上还是原来那一份。
//! 3. 替换之前再读一次本体：和调用的一方上一次读的版本不一样（这一瞬间有人手改），放弃这一次（[`WriteError::Changed`]），
//!    由它重读再来。这缩小了窗口，关不死：编辑器不加锁。
//! 4. Windows 上本体正被别的程序开着、改名失败的，歇 20 毫秒再试，最多 5 次。

use std::collections::BTreeSet;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::durable::{create_dir, create_temp_with, discard, sync_dir, temp_name};

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
    /// 开头有没有 UTF-8 BOM：写回时照样加回（施工 8-3）。
    pub bom: bool,
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
    let stripped = text.strip_prefix(BOM);
    Ok(ConfigText {
        text: stripped.unwrap_or(text).to_string(),
        version: version(bytes),
        bom: stripped.is_some(),
    })
}

/// 一份字写回去的字节：原来开头有 BOM 的照样加回。
pub fn bytes(text: &str, bom: bool) -> Vec<u8> {
    match bom {
        true => format!("{BOM}{text}").into_bytes(),
        false => text.as_bytes().to_vec(),
    }
}

/// 版本：`sha256:` 加整份字节的 SHA-256，64 位小写十六进制。
pub fn version(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("sha256:{hex}")
}

/// 顺着链接最多找几层（第五条第 4 条）。
const LINKS: usize = 40;

/// Windows 上改名失败最多再试几次、每次歇多久（第五条第 7 条）。
const RENAME_TRIES: u32 = 5;
const RENAME_PAUSE: std::time::Duration = std::time::Duration::from_millis(20);

/// 写不成一份配置文件。
#[derive(Debug)]
pub enum WriteError {
    /// 替换之前再读一次，和上一次读的不一样：这一瞬间有人手改了。什么都没写。
    Changed,
    /// 顺着链接找不到本体（绕圈、超过 40 层），写不进、同步不了、改不了名。什么都没变。
    Io(io::Error),
}

impl fmt::Display for WriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WriteError::Changed => write!(f, "changed while writing"),
            WriteError::Io(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for WriteError {}

impl From<io::Error> for WriteError {
    fn from(error: io::Error) -> WriteError {
        WriteError::Io(error)
    }
}

/// 把 `path` 这一份配置文件整份写成 `content`。`read` 是调用的一方上一次读到的版本（[`version`]），文件那时还没有的是
/// 空的：替换之前本体和它对不上，放弃。没有的目录建上。
///
/// # Errors
///
/// 这一瞬间有人手改了（[`WriteError::Changed`]）；链接绕圈、太深；写不进、同步不了、改不了名。都是什么都没变。
pub fn write(path: &Path, content: &[u8], read: Option<&str>) -> Result<(), WriteError> {
    write_with(path, content, read, Mode::Keep, &mut |_| Ok(()))
}

/// 写成的文件是什么权限。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    /// 带上原文件的权限位，新文件照系统默认（配置文件）。
    Keep,
    /// Unix 上一律 0600，临时文件建的时候就是（密钥文件，施工 8-5）。
    Private,
}

/// 同 [`write()`]，权限照 `mode`；改名之前先叫一声 `before_rename`（交给它临时文件在哪）：测试照它在那一瞬间手改文件、
/// 装作崩了。
pub(crate) fn write_with(
    path: &Path,
    content: &[u8],
    read: Option<&str>,
    mode: Mode,
    before_rename: &mut dyn FnMut(&Path) -> io::Result<()>,
) -> Result<(), WriteError> {
    let target = real(path)?;
    let (Some(dir), Some(name)) = (target.parent(), target.file_name()) else {
        return Err(WriteError::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{} is not a file in a directory", target.display()),
        )));
    };
    create_dir(dir)?;
    let name = name.to_string_lossy();
    let (temp, file) = create_temp_with(dir, || temp_name(&name), mode == Mode::Private)?;
    let written = replace(file, content, &temp, &target, read, mode, before_rename);
    if written.is_err() {
        discard(&temp);
    }
    written?;
    Ok(sync_dir(dir)?)
}

/// 写进临时文件、同步、带上原来的权限位（`Private` 的不带，建的时候就是 0600）、关上；再读一次本体、对得上才改名盖上。
fn replace(
    mut file: File,
    content: &[u8],
    temp: &Path,
    target: &Path,
    read: Option<&str>,
    mode: Mode,
    before_rename: &mut dyn FnMut(&Path) -> io::Result<()>,
) -> Result<(), WriteError> {
    file.write_all(content)?;
    file.sync_data()?;
    if mode == Mode::Keep
        && let Ok(metadata) = fs::metadata(target)
    {
        file.set_permissions(metadata.permissions())?;
    }
    drop(file);
    before_rename(temp)?;
    let now = match fs::read(target) {
        Ok(bytes) => Some(version(&bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    if now.as_deref() != read {
        return Err(WriteError::Changed);
    }
    Ok(rename(temp, target)?)
}

/// 改名盖上本体。Windows 上本体正被别的程序开着的，歇一下再试。
fn rename(temp: &Path, target: &Path) -> io::Result<()> {
    let mut tries = 0;
    loop {
        match fs::rename(temp, target) {
            Err(_) if cfg!(windows) && tries < RENAME_TRIES => {
                tries += 1;
                std::thread::sleep(RENAME_PAUSE);
            }
            done => return done,
        }
    }
}

/// 顺着链接找到本体：相对的照链接所在的目录接，最多 40 层，绕圈的报错。本体还没有的，就是它该在的地方。
fn real(path: &Path) -> io::Result<PathBuf> {
    let mut at = path.to_path_buf();
    let mut seen = BTreeSet::new();
    for _ in 0..LINKS {
        let metadata = match fs::symlink_metadata(&at) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(at),
            Err(error) => return Err(error),
        };
        if !metadata.file_type().is_symlink() {
            return Ok(at);
        }
        if !seen.insert(at.clone()) {
            break;
        }
        let pointed = fs::read_link(&at)?;
        at = match at.parent() {
            Some(parent) if pointed.is_relative() => parent.join(pointed),
            _ => pointed,
        };
    }
    Err(io::Error::other(format!(
        "{} links round in a circle or more than {LINKS} deep",
        path.display()
    )))
}

#[cfg(test)]
mod tests;
