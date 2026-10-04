//! 本机传输（`docs/designs/07-存储.md` 第二节、第十节，`04-核心协议.md` 第二节，施工 3-8 下）：核心在
//! 本机的套接字上等连接，头连过去。
//!
//! 核心起来时（[`open`]）：
//!
//! 1. 拿 `run/core.lock` 的锁：一个数据根上只跑一个核心，拿不到就是已经有一个在跑；
//! 2. 算出套接字放哪：一个数据根一个位置，名字里带数据根的指纹，几个数据根不撞；
//! 3. 换一个本机令牌，写进 `run/token`：本机的头握手时出示它；
//! 4. 在套接字上等连接：套接字放在只有自己能进的目录里，上一个核心崩了留下的旧套接字文件删掉；
//! 5. 实际位置写进 `run/socket`。
//!
//! 头（[`connect`]）读 `run/socket`，核对套接字所在的目录只有自己能进，连过去，再现读本机令牌。核心没在跑，
//! 头用 [`connect_or_start`] 把它拉起来，等它写来 [`Ready`] 那一行再连（施工 3-9 上）。
//!
//! Unix 上是 Unix 域套接字；Windows 上是命名管道 `\\.\pipe\miyu-<指纹>`，只对本人开放，头连上以后核对
//! 另一头的进程是自己的（施工 3-8 补）。

mod error;
mod files;
mod listener;
mod lock;
mod place;
mod ready;
mod start;
#[cfg(test)]
mod test_support;

#[cfg(unix)]
mod unix;
#[cfg(unix)]
use unix as sys;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as sys;

pub use error::{ConnectError, OpenError, StartError};
pub use listener::{Connection, Listener};
pub use lock::Lock;
pub use place::{Dirs, fingerprint};
pub use ready::Ready;
pub use start::{connect_or_start, connect_or_start_bare, spawn_detached};

use std::fmt;
use std::io;

use miyu_store::root::DataRoot;

/// 核心起来了：在套接字上等连接，手里有这一次的本机令牌。
pub struct Opened {
    /// 在套接字上等连接。丢掉它，套接字文件删掉，锁放开。
    pub listener: Listener,
    /// 这一次的本机令牌：头握手时要出示它。
    pub token: String,
}

/// 令牌不打出来。
impl fmt::Debug for Opened {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Opened")
            .field("listener", &self.listener)
            .finish_non_exhaustive()
    }
}

/// 核心起来：拿锁，算出套接字放哪，换本机令牌，在套接字上等连接，把实际位置写进 `run/socket`。
/// 数据根要已经建好骨架。
///
/// 要在 tokio 运行时里调：套接字要登记到它上面。
///
/// # Errors
///
/// 已经有一个核心在跑；套接字哪里都放不下，要放的目录不是只有自己能进，或者位置上有别的东西；
/// 读写出错。
///
/// # Panics
///
/// 不在 tokio 运行时里。
pub fn open(root: &DataRoot, dirs: &Dirs) -> Result<Opened, OpenError> {
    open_locked(root, dirs, Lock::acquire(root)?)
}

/// 锁已经拿到了，接着起来：核心进程先拿锁、再装运行日志，免得两个核心写同一份日志（施工 3-9 上）。
///
/// 要在 tokio 运行时里调。
///
/// # Errors
///
/// 同 [`open`]，没有「已经在跑」。
///
/// # Panics
///
/// 不在 tokio 运行时里。
pub fn open_locked(root: &DataRoot, dirs: &Dirs, lock: Lock) -> Result<Opened, OpenError> {
    let path = place::locate(root, dirs)?;
    let token = files::renew_token(root)?;
    let own_dir = place::own_dir(&path, dirs);
    let listener = Listener::new(sys::bind(&path)?, path, own_dir, lock);
    files::write_location(root, listener.path())?;
    tracing::info!(target: "miyu::ipc", socket = %listener.path().display(), "listening");
    Ok(Opened { listener, token })
}

/// 头连核心：读 `run/socket`，核对套接字所在的目录只有自己能进（Windows 上核对管道另一头的进程是自己的），
/// 连过去，再现读本机令牌。先连后读：核心刚换过令牌的话，读到的是新的。
///
/// # Errors
///
/// 核心没在跑；套接字所在的目录不是只有自己能进，或者管道另一头不是自己的进程；读写出错。
pub async fn connect(root: &DataRoot) -> Result<(Connection, String), ConnectError> {
    let connection = connect_bare(root).await?;
    let token = files::read_token(root)?;
    Ok((connection, token))
}

/// 同 [`connect`]，只是不读本机令牌（施工 W-8，`web-module.md`「起草时定的」第 4 条）：网页软件转发浏览器的连接用它，
/// 浏览器的凭据由页面在握手时自己出示，网页软件的代码里拿不到本机令牌。核对目录、核对管道另一头照旧。
///
/// # Errors
///
/// 核心没在跑；套接字所在的目录不是只有自己能进，或者管道另一头不是自己的进程；读写出错。
pub async fn connect_bare(root: &DataRoot) -> Result<Connection, ConnectError> {
    let path = match files::read_location(root) {
        Ok(path) => path,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(ConnectError::NotRunning);
        }
        Err(error) => return Err(ConnectError::Io(error)),
    };
    let stream = sys::connect(&path).await?;
    Ok(Connection::new(stream))
}
