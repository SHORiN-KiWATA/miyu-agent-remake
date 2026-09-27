//! 把她给的路径换成真实的位置（`11-权限与沙盒.md` 第七节，施工 4-3 上）：相对路径照这一轮的工作目录
//! 接上，`~` 开头的当家目录；符号链接、Windows 的目录联接都照它们指向的地方算；还不存在的，照最近的、
//! 已经存在的上级目录算。

use std::fmt;
use std::io;
use std::path::{Component, Path, PathBuf};

/// 换不成真实的位置。
#[derive(Debug)]
pub enum ResolveError {
    /// 以 `~` 开头，可家目录读不出来。
    NoHome,
    /// 还不存在的那几段里有 `..`：还不存在的目录往上走，说不清落在哪。
    ParentOfMissing,
    /// 路径上有一个链接，指向的地方不存在：照它往下写，会写到链接指的别处去。
    DanglingLink,
    /// 连根都找不到存在的上级目录，或者读上级目录时出了错（例如没有权限）。
    Io(io::Error),
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResolveError::NoHome => f.write_str("the home directory is not known"),
            ResolveError::ParentOfMissing => {
                f.write_str("'..' after a directory that does not exist")
            }
            ResolveError::DanglingLink => f.write_str("a link on the path points nowhere"),
            ResolveError::Io(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ResolveError {}

/// 把 `input` 换成真实的位置：相对的照 `cwd` 接上，`~` 开头的照 `home` 接上。
///
/// # Errors
///
/// 以 `~` 开头可 `home` 是空的；还不存在的那几段里有 `..`；读上级目录时出了错。
pub fn resolve(cwd: &Path, home: Option<&Path>, input: &str) -> Result<PathBuf, ResolveError> {
    let path = match tilde(input) {
        Some(rest) => home.ok_or(ResolveError::NoHome)?.join(rest),
        None => cwd.join(input),
    };
    match std::fs::canonicalize(&path) {
        Ok(real) => Ok(real),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            dangling(&path)?;
            missing(&path)
        }
        Err(error) => Err(ResolveError::Io(error)),
    }
}

/// `~` 开头的：交回 `~` 后面那一截（去掉紧跟着的分隔符）。只有 `~` 自己，或者 `~` 后面紧跟分隔符的才算：
/// `~alice`、`a/~` 照原样。
fn tilde(input: &str) -> Option<&str> {
    let rest = input.strip_prefix('~')?;
    if rest.is_empty() {
        return Some("");
    }
    let mut chars = rest.chars();
    match chars.next() {
        Some('/') => Some(chars.as_str()),
        Some('\\') if cfg!(windows) => Some(chars.as_str()),
        _ => None,
    }
}

/// 还不存在的路径：从长往短试它的上级目录，最近的、已经存在的那一层换成真实的位置，再接上后面几段。
fn missing(path: &Path) -> Result<PathBuf, ResolveError> {
    let components: Vec<Component<'_>> = path.components().collect();
    for keep in (1..components.len()).rev() {
        let prefix: PathBuf = components[..keep].iter().collect();
        match std::fs::canonicalize(&prefix) {
            Ok(real) => return append(real, &components[keep..]),
            Err(error) if error.kind() == io::ErrorKind::NotFound => dangling(&prefix)?,
            Err(error) => return Err(ResolveError::Io(error)),
        }
    }
    Err(ResolveError::Io(io::Error::from(io::ErrorKind::NotFound)))
}

/// 换不成真实位置的 `path` 自己却在：它是一个指向不存在处的链接。
fn dangling(path: &Path) -> Result<(), ResolveError> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Err(ResolveError::DanglingLink),
        Err(_) => Ok(()),
    }
}

/// 在已经存在的 `real` 后面接上还不存在的几段。里面有 `..` 的不认。
fn append(mut real: PathBuf, rest: &[Component<'_>]) -> Result<PathBuf, ResolveError> {
    for component in rest {
        match component {
            Component::Normal(part) => real.push(part),
            Component::CurDir => {}
            _ => return Err(ResolveError::ParentOfMissing),
        }
    }
    Ok(real)
}
