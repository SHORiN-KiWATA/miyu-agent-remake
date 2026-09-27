//! Windows 上的本机传输（`docs/designs/04-核心协议.md` 第二节「各平台的坑」，施工 3-8 补）：命名管道，只对
//! 本人开放。安全描述符、第一个实例、问出另一头是谁要调 Windows 的安全接口，在放开了 `unsafe` 的
//! `miyu-pipe` 里；这里只管等连接、连过去。头连上以后核对管道另一头的进程是自己的，免得核心没在跑的
//! 时候，别的用户先占了这个名字冒充核心。

use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient, NamedPipeServer};
use windows_sys::Win32::Foundation::ERROR_PIPE_BUSY;

use crate::error::{ConnectError, OpenError};

/// 管道忙的时候歇多久再连。
const BUSY_PAUSE: Duration = Duration::from_millis(50);

/// 管道忙最多再连几次：一共 5 秒。
const BUSY_TRIES: u32 = 100;

/// 连接的一头：核心接到的是服务端，头连上的是客户端。
pub(crate) enum Stream {
    /// 核心这一头。
    Server(NamedPipeServer),
    /// 头那一头。
    Client(NamedPipeClient),
}

/// 在等连接的管道：手里总有一个等着的实例。
pub(crate) struct Socket {
    /// 管道名。
    name: PathBuf,
    /// 等着的实例。
    waiting: NamedPipeServer,
}

impl Socket {
    /// 等下一个连接：连上了，先建好下一个实例等着，再把连上的这个交出去。等连接出了错的，这个实例就
    /// 用不了了，也换上新建的、扔掉它：不换的话，以后每次接都卡在同一个错上。
    pub(crate) async fn accept(&mut self) -> io::Result<Stream> {
        let connected = self.waiting.connect().await;
        let next = miyu_pipe::create(self.name.as_os_str(), false)?;
        let this = std::mem::replace(&mut self.waiting, next);
        connected?;
        Ok(Stream::Server(this))
    }
}

/// 建管道的第一个实例。名字已经被别的程序占了（拒绝访问），不动它。
///
/// # Errors
///
/// 名字被占了；建不成。
pub(crate) fn bind(path: &Path) -> Result<Socket, OpenError> {
    match miyu_pipe::create(path.as_os_str(), true) {
        Ok(waiting) => Ok(Socket {
            name: path.to_path_buf(),
            waiting,
        }),
        Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
            Err(OpenError::Occupied(path.to_path_buf()))
        }
        Err(error) => Err(OpenError::Io(error)),
    }
}

/// 连管道：忙就歇一下再连；连上以后核对另一头的进程是自己的。
///
/// # Errors
///
/// 没有这个管道（核心没在跑）；另一头不是自己的进程；读写出错。
pub(crate) async fn connect(path: &Path) -> Result<Stream, ConnectError> {
    let mut tries = 0;
    let pipe = loop {
        match ClientOptions::new().open(path) {
            Ok(pipe) => break pipe,
            Err(error)
                if error.raw_os_error() == Some(ERROR_PIPE_BUSY as i32) && tries < BUSY_TRIES =>
            {
                tries += 1;
                tokio::time::sleep(BUSY_PAUSE).await;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(ConnectError::NotRunning);
            }
            Err(error) => return Err(ConnectError::Io(error)),
        }
    };
    if miyu_pipe::server_user(&pipe)? != miyu_pipe::current_user()? {
        return Err(ConnectError::NotPrivate(path.to_path_buf()));
    }
    Ok(Stream::Client(pipe))
}

/// 走的时候：没有套接字文件要删，管道的句柄都关了它就没了。
pub(crate) fn remove(_path: &Path) {}

/// 拉起的核心跟终端脱开：不带控制台窗口，自成一组，终端里按 Ctrl+C 打不到它。
pub(crate) fn detach(command: &mut std::process::Command) {
    use std::os::windows::process::CommandExt;
    use windows_sys::Win32::System::Threading::{CREATE_NEW_PROCESS_GROUP, DETACHED_PROCESS};
    command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
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
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Stream::Server(pipe) => Pin::new(pipe).poll_read(cx, buf),
            Stream::Client(pipe) => Pin::new(pipe).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for Stream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match self.get_mut() {
            Stream::Server(pipe) => Pin::new(pipe).poll_write(cx, buf),
            Stream::Client(pipe) => Pin::new(pipe).poll_write(cx, buf),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Stream::Server(pipe) => Pin::new(pipe).poll_flush(cx),
            Stream::Client(pipe) => Pin::new(pipe).poll_flush(cx),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Stream::Server(pipe) => Pin::new(pipe).poll_shutdown(cx),
            Stream::Client(pipe) => Pin::new(pipe).poll_shutdown(cx),
        }
    }
}
