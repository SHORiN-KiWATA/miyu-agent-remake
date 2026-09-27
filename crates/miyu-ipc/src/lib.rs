//! 本机传输（`docs/designs/07-存储.md` 第二节、第十节，`04-核心协议.md` 第二节，施工 3-8 下）：核心在
//! 本机的套接字上等连接，头连过去。
//!
//! 核心起来时（[`open`]）：
//!
//! 1. 算出套接字放哪：一个数据根一个位置，名字里带数据根的指纹，几个数据根不撞；
//! 2. 拿 `run/core.lock` 的锁：一个数据根上只跑一个核心，拿不到就是已经有一个在跑；
//! 3. 换一个本机令牌，写进 `run/token`：本机的头握手时出示它；
//! 4. 在套接字上等连接：套接字放在只有自己能进的目录里，上一个核心崩了留下的旧套接字文件删掉；
//! 5. 实际位置写进 `run/socket`。
//!
//! 头（[`connect`]）读 `run/socket`，核对套接字所在的目录只有自己能进，连过去，再现读本机令牌。
//!
//! Unix 上是 Unix 域套接字。Windows 的命名管道随施工 3-8（补），现在监听回「还不支持」。

mod error;
mod files;
mod listener;
mod lock;
mod place;
#[cfg(test)]
mod test_support;

#[cfg(not(unix))]
mod other;
#[cfg(not(unix))]
use other as sys;
#[cfg(unix)]
mod unix;
#[cfg(unix)]
use unix as sys;

pub use error::{ConnectError, OpenError};
pub use listener::{Connection, Listener};
pub use place::{Dirs, fingerprint};

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

/// 核心起来：算出套接字放哪，拿锁，换本机令牌，在套接字上等连接，把实际位置写进 `run/socket`。
/// 数据根要已经建好骨架。
///
/// 要在 tokio 运行时里调：套接字要登记到它上面。
///
/// # Errors
///
/// 已经有一个核心在跑；套接字哪里都放不下，要放的目录不是只有自己能进，或者位置上有别的东西；
/// 这个平台上还不能在本机监听；读写出错。
///
/// # Panics
///
/// 不在 tokio 运行时里。
pub fn open(root: &DataRoot, dirs: &Dirs) -> Result<Opened, OpenError> {
    let path = place::locate(root, dirs)?;
    let lock = lock::Lock::acquire(root)?;
    let token = files::renew_token(root)?;
    let listener = Listener::new(sys::bind(&path)?, path, lock);
    files::write_location(root, listener.path())?;
    tracing::info!(target: "miyu::ipc", socket = %listener.path().display(), "listening");
    Ok(Opened { listener, token })
}

/// 头连核心：读 `run/socket`，核对套接字所在的目录只有自己能进，连过去，再现读本机令牌。先连后读：
/// 核心刚换过令牌的话，读到的是新的。
///
/// # Errors
///
/// 核心没在跑；套接字所在的目录不是只有自己能进；读写出错。
pub async fn connect(root: &DataRoot) -> Result<(Connection, String), ConnectError> {
    let path = match files::read_location(root) {
        Ok(path) => path,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(ConnectError::NotRunning);
        }
        Err(error) => return Err(ConnectError::Io(error)),
    };
    let stream = sys::connect(&path).await?;
    let token = files::read_token(root)?;
    Ok((Connection::new(stream), token))
}
