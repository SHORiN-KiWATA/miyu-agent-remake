//! 在套接字上等连接的一头，和连上以后的一个连接。和平台有关的在 `unix`（以后还有 Windows 的命名
//! 管道），这里只是一层壳：协议端点只认异步的字节流，不管它从哪来。

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use crate::lock::Lock;
use crate::sys;

/// 核心在套接字上等连接。丢掉它：套接字文件删掉，锁放开。先删文件再放锁：放了锁，下一个核心
/// 马上就可能在同一个位置上绑。
pub struct Listener {
    /// 在等连接的套接字。
    socket: sys::Socket,
    /// 套接字在哪。
    path: PathBuf,
    /// 单实例锁。放在最后：字段照声明的先后丢，锁最后放。
    _lock: Lock,
}

impl Listener {
    /// 套接字绑好了，锁拿着了。
    pub(crate) fn new(socket: sys::Socket, path: PathBuf, lock: Lock) -> Listener {
        Listener {
            socket,
            path,
            _lock: lock,
        }
    }

    /// 等下一个连接。
    ///
    /// # Errors
    ///
    /// 接不了，例如打开的文件太多了。
    pub async fn accept(&self) -> io::Result<Connection> {
        self.socket.accept().await.map(Connection::new)
    }

    /// 套接字在哪。
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_file(&self.path) {
            tracing::debug!(target: "miyu::ipc", error = %error, "socket file not removed");
        }
    }
}

impl fmt::Debug for Listener {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Listener")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

/// 连上以后的一个连接：Unix 上是套接字的一头。读写都是异步的字节流，交给协议端点。
pub struct Connection(sys::Stream);

impl Connection {
    /// 包一层。
    pub(crate) fn new(stream: sys::Stream) -> Connection {
        Connection(stream)
    }
}

impl fmt::Debug for Connection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Connection").finish_non_exhaustive()
    }
}

impl AsyncRead for Connection {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().0).poll_read(cx, buf)
    }
}

impl AsyncWrite for Connection {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.get_mut().0).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().0).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().0).poll_shutdown(cx)
    }
}
