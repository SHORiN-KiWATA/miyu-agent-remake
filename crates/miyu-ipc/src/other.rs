//! 别的平台（Windows）：命名管道随施工 3-8（补），现在在本机监听、连接都回「还不支持」。套接字和
//! 连接的类型造不出来，用到它们的代码照样编得过。

use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use crate::error::{ConnectError, OpenError};

/// 连接：还造不出来。
pub(crate) enum Stream {}

/// 在等连接的套接字：还造不出来。
pub(crate) enum Socket {}

impl Socket {
    /// 等下一个连接：造不出来的，走不到这里。
    pub(crate) async fn accept(&self) -> io::Result<Stream> {
        match *self {}
    }
}

/// 还不能监听。
pub(crate) fn bind(_path: &Path) -> Result<Socket, OpenError> {
    Err(OpenError::Unsupported)
}

/// 还不能连。
pub(crate) async fn connect(_path: &Path) -> Result<Stream, ConnectError> {
    Err(ConnectError::Unsupported)
}

/// 没有用户编号。
pub(crate) fn uid() -> Option<u32> {
    None
}

/// 不用 `$XDG_RUNTIME_DIR`。
pub(crate) fn runtime_dir(_value: Option<OsString>) -> Option<PathBuf> {
    None
}

/// 新建文件：已经有的算错。靠用户目录本身的访问控制（`07-存储.md` 第二节）。
pub(crate) fn create_private(path: &Path) -> io::Result<File> {
    OpenOptions::new().write(true).create_new(true).open(path)
}

/// 文件里的字节就是路径，要是 UTF-8。
pub(crate) fn path_from_bytes(bytes: Vec<u8>) -> io::Result<PathBuf> {
    String::from_utf8(bytes)
        .map(PathBuf::from)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

impl AsyncRead for Stream {
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        _buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match *self {}
    }
}

impl AsyncWrite for Stream {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        _buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match *self {}
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match *self {}
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match *self {}
    }
}
