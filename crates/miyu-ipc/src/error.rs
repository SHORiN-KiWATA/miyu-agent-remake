//! 核心起不来、头连不上的几种情形。

use std::fmt;
use std::io;
use std::path::PathBuf;

/// 核心起不来。
#[derive(Debug)]
pub enum OpenError {
    /// 这个数据根上已经有一个核心在跑：`run/core.lock` 在它手里。
    Running,
    /// 套接字的路径太长：本该放的位置、临时目录下都放不下。带着本该放的位置。
    TooLong(PathBuf),
    /// 套接字要放的目录不是自己的，或者别人也能进：不用它。
    NotPrivate(PathBuf),
    /// 套接字的位置上有别的东西：不是套接字的文件，或者别的程序正在听。不动它。
    Occupied(PathBuf),
    /// 这个平台上还不能在本机监听：Windows 的命名管道随施工 3-8（补）。
    Unsupported,
    /// 读写出错。
    Io(io::Error),
}

impl fmt::Display for OpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpenError::Running => write!(f, "这个数据根上已经有一个核心在跑"),
            OpenError::TooLong(path) => write!(
                f,
                "套接字的路径太长，放不下：{}；临时目录下也放不下",
                path.display()
            ),
            OpenError::NotPrivate(dir) => write!(
                f,
                "{} 不是自己的，或者别人也能进：套接字不放在这里",
                dir.display()
            ),
            OpenError::Occupied(path) => write!(
                f,
                "{} 上有别的东西：不是套接字的文件，或者别的程序正在听。不动它",
                path.display()
            ),
            OpenError::Unsupported => write!(f, "这个平台上还不能在本机监听"),
            OpenError::Io(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for OpenError {}

impl From<io::Error> for OpenError {
    fn from(error: io::Error) -> OpenError {
        OpenError::Io(error)
    }
}

/// 头连不上核心。
#[derive(Debug)]
pub enum ConnectError {
    /// 核心没在跑：`run/socket` 没有，或者那里连不上。
    NotRunning,
    /// 套接字所在的目录不是自己的，或者别人也能进：不连，免得把本机令牌交给冒充核心的人。
    NotPrivate(PathBuf),
    /// 这个平台上还不能在本机连：Windows 的命名管道随施工 3-8（补）。
    Unsupported,
    /// 读写出错。
    Io(io::Error),
}

impl fmt::Display for ConnectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectError::NotRunning => write!(f, "核心没在跑"),
            ConnectError::NotPrivate(dir) => write!(
                f,
                "{} 不是自己的，或者别人也能进：不连这里的套接字",
                dir.display()
            ),
            ConnectError::Unsupported => write!(f, "这个平台上还不能在本机连核心"),
            ConnectError::Io(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for ConnectError {}

impl From<io::Error> for ConnectError {
    fn from(error: io::Error) -> ConnectError {
        ConnectError::Io(error)
    }
}
