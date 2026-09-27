//! Unix 上的本机传输（`docs/designs/04-核心协议.md` 第二节「各平台的坑」）：Unix 域套接字，放在只有
//! 自己能进的目录里（权限 0700）。核心绑之前、头连之前都核对这个目录：是自己的、别人进不来，免得
//! 别人抢先建一个同名的套接字冒充核心。

use std::ffi::OsString;
use std::fs::{self, File, Metadata, OpenOptions};
use std::io;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use crate::error::{ConnectError, OpenError};

/// 套接字的一头。
pub(crate) type Stream = tokio::net::UnixStream;

/// 在等连接的套接字。
pub(crate) struct Socket(tokio::net::UnixListener);

impl Socket {
    /// 等下一个连接。
    pub(crate) async fn accept(&self) -> io::Result<Stream> {
        self.0.accept().await.map(|(stream, _)| stream)
    }
}

/// 在 `path` 上绑套接字。所在的目录没有的，建成 0700（上一层要已经在）；核对它只有自己能进。
/// 位置上是旧套接字（连不上）的删掉；有别的东西的不动。
///
/// # Errors
///
/// 目录不是只有自己能进；位置上有别的东西；读写出错。
pub(crate) fn bind(path: &Path) -> Result<Socket, OpenError> {
    let dir = parent(path);
    make_private(dir)?;
    if !private(dir)? {
        return Err(OpenError::NotPrivate(dir.to_path_buf()));
    }
    match probe(path)? {
        Found::Nothing => {}
        Found::Stale => {
            fs::remove_file(path)?;
            tracing::info!(target: "miyu::ipc", socket = %path.display(), "stale socket removed");
        }
        Found::Taken => return Err(OpenError::Occupied(path.to_path_buf())),
    }
    Ok(Socket(tokio::net::UnixListener::bind(path)?))
}

/// 连 `path` 上的套接字：先核对所在的目录只有自己能进。
///
/// # Errors
///
/// 连不上（核心没在跑）；目录不是只有自己能进；读写出错。
pub(crate) async fn connect(path: &Path) -> Result<Stream, ConnectError> {
    let dir = parent(path);
    match private(dir) {
        Ok(true) => {}
        Ok(false) => return Err(ConnectError::NotPrivate(dir.to_path_buf())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(ConnectError::NotRunning);
        }
        Err(error) => return Err(ConnectError::Io(error)),
    }
    match tokio::net::UnixStream::connect(path).await {
        Ok(stream) => Ok(stream),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
            ) =>
        {
            Err(ConnectError::NotRunning)
        }
        Err(error) => Err(ConnectError::Io(error)),
    }
}

/// 走的时候删掉套接字文件。
pub(crate) fn remove(path: &Path) {
    if let Err(error) = fs::remove_file(path) {
        tracing::debug!(target: "miyu::ipc", error = %error, "socket file not removed");
    }
}

/// 拉起的核心跟终端脱开：自成一个进程组，终端里按 Ctrl+C 打不到它。
pub(crate) fn detach(command: &mut std::process::Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

/// 有效用户编号。
pub(crate) fn uid() -> Option<u32> {
    Some(rustix::process::geteuid().as_raw())
}

/// `$XDG_RUNTIME_DIR`：要是绝对路径、只有自己能进的目录（XDG 规范这么要求），不合的当没设，套接字
/// 改放数据根的 `run/`。例如 `su` 成别的用户以后，它还指着原来那个人的目录。
pub(crate) fn runtime_dir(value: Option<OsString>) -> Option<PathBuf> {
    let dir = PathBuf::from(value.filter(|value| !value.is_empty())?);
    let usable = dir.is_absolute()
        && fs::metadata(&dir).is_ok_and(|meta| meta.is_dir() && owned_alone(&meta));
    if !usable {
        tracing::warn!(target: "miyu::ipc", dir = %dir.display(), "XDG_RUNTIME_DIR not usable, using run/");
    }
    usable.then_some(dir)
}

/// 新建只有自己能读写的文件（0600）：已经有的算错。
pub(crate) fn create_private(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
}

/// 文件里的字节就是路径。
pub(crate) fn path_from_bytes(bytes: Vec<u8>) -> io::Result<PathBuf> {
    Ok(OsString::from_vec(bytes).into())
}

/// 套接字所在的目录。
fn parent(path: &Path) -> &Path {
    path.parent().unwrap_or(path)
}

/// 建只有自己能进的目录：0700，上一层要已经在。已经有的不动，交给 [`private`] 核对。
fn make_private(dir: &Path) -> io::Result<()> {
    match fs::DirBuilder::new().mode(0o700).create(dir) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(error),
    }
}

/// 目录是不是只有自己能进：是目录（不是链接），属主是自己，组和别人一点权限都没有。
fn private(dir: &Path) -> io::Result<bool> {
    let meta = fs::symlink_metadata(dir)?;
    Ok(meta.is_dir() && owned_alone(&meta))
}

/// 属主是自己，组和别人一点权限都没有。
fn owned_alone(meta: &Metadata) -> bool {
    alone(meta.uid(), meta.mode(), rustix::process::geteuid().as_raw())
}

/// 属主是 `me`，组和别人一点权限都没有。只看数字：别人的目录在测试里造不出来，这样也测得到。
fn alone(owner: u32, mode: u32, me: u32) -> bool {
    owner == me && mode & 0o077 == 0
}

/// 套接字的位置上有什么。
enum Found {
    /// 什么都没有。
    Nothing,
    /// 旧套接字：上一个核心崩了没删，连不上。
    Stale,
    /// 别的东西：不是套接字的文件，或者连得上的套接字（别的程序正在听）。
    Taken,
}

/// 看看套接字的位置上有什么。连得上的不删：例如另一个数据根的指纹撞了，它的核心正在用。
fn probe(path: &Path) -> io::Result<Found> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Found::Nothing),
        Err(error) => return Err(error),
    };
    if !meta.file_type().is_socket() {
        return Ok(Found::Taken);
    }
    match std::os::unix::net::UnixStream::connect(path) {
        Ok(_) => Ok(Found::Taken),
        Err(error) if error.kind() == io::ErrorKind::ConnectionRefused => Ok(Found::Stale),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Found::Nothing),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests;
