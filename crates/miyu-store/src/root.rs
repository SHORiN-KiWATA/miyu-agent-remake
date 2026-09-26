//! 数据根和缓存目录（`docs/designs/07-存储.md` 第二节）：在哪，第一次用时怎么建。
//!
//! 数据根装着全部真相和派生数据，一个数据根上只跑一个核心，默认在家目录的 `.miyu` 里；缓存目录
//! 装模型文件这类大缓存，整台机器共用，换了数据根也不用重新下载。两样都照一份环境快照（[`Env`]）
//! 找。数据根的顶层有一个标记文件，认不出是自己的数据根就不碰它：旧版 Miyu 也放在 `~/.miyu`。

use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use miyu_kernel::id::{AccountId, SessionId};

use crate::env::{Env, Platform};

/// 第一次用时建的四个顶层目录：系统区、家目录、状态区、运行时（`07-存储.md` 第二节）。
/// 别的用到时再建。
const SKELETON: [&str; 4] = ["system", "home", "state", "run"];

/// 标记文件：它在，这个目录才是 Miyu 的数据根（`07-存储.md` 第二节「认得出自己的数据根才动它」）。
const MARKER: &str = ".miyu-root";

/// 标记文件里写的一行：给翻到它的人看。现在只认文件在不在。
const MARKER_TEXT: &str = "This directory is a Miyu data root (layout 1).\n";

/// 找不到数据根、缓存目录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RootError {
    /// `MIYU_HOME` 写的是相对路径：它跟着当前目录变，同一个人在两个目录里启动，会找到两个
    /// 数据根。
    RelativeMiyuHome(PathBuf),
    /// 家目录找不到（Linux、macOS）。
    NoHome,
    /// `LOCALAPPDATA` 没有，或者不是绝对路径（Windows 的缓存目录要它）。
    NoLocalAppData,
}

impl fmt::Display for RootError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RootError::RelativeMiyuHome(path) => {
                write!(f, "MIYU_HOME 要写绝对路径，写的是 {}", path.display())
            }
            RootError::NoHome => write!(f, "找不到家目录"),
            RootError::NoLocalAppData => write!(f, "找不到 LOCALAPPDATA，或者它不是绝对路径"),
        }
    }
}

impl std::error::Error for RootError {}

/// 一个数据根。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataRoot {
    path: PathBuf,
}

impl DataRoot {
    /// 照快照找数据根：`MIYU_HOME` 设了就是它，不然是家目录的 `.miyu`，三个平台一样
    /// （`07-存储.md` 第二节「默认位置」「怎么找」）。
    ///
    /// # Errors
    ///
    /// `MIYU_HOME` 是相对路径；要用家目录时找不到。
    pub fn locate(env: &Env) -> Result<DataRoot, RootError> {
        if let Some(miyu_home) = set(&env.miyu_home) {
            let path = PathBuf::from(miyu_home);
            return match path.is_absolute() {
                true => Ok(DataRoot { path }),
                false => Err(RootError::RelativeMiyuHome(path)),
            };
        }
        Ok(DataRoot {
            path: home(env)?.join(".miyu"),
        })
    }

    /// 数据根本身。
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 系统区：管理员维护，对成员只读。
    pub fn system(&self) -> PathBuf {
        self.path.join("system")
    }

    /// 放各个账号家目录的地方：`home/`。
    pub fn homes(&self) -> PathBuf {
        self.path.join("home")
    }

    /// 一个会话的目录：`home/<账号>/sessions/<会话编号>/`（`07-存储.md` 第三节）。
    pub fn session_dir(&self, account: &AccountId, session: &SessionId) -> PathBuf {
        self.homes()
            .join(account.as_str())
            .join("sessions")
            .join(session.as_str())
    }

    /// 状态区：派生的全局索引、用量总表、运行日志。
    pub fn state(&self) -> PathBuf {
        self.path.join("state")
    }

    /// 运行时：锁文件、套接字、本机令牌。
    pub fn run(&self) -> PathBuf {
        self.path.join("run")
    }

    /// 建骨架：先认标记。目录不存在、是空的，先写下标记；有标记的照常；不是空的又没有标记的，
    /// 不是 Miyu 的数据根，里面什么都不建。然后四个顶层目录，缺的才建，建两次也不出错。
    ///
    /// Unix 上新建的权限 0700，只有本人能进；已经有的不改：数据根可能是人自己建、自己设的，权限
    /// 不对由 `miyu doctor` 报告（`22-命令行.md` 第五节）。Windows 上靠用户目录本身的访问控制。
    ///
    /// # Errors
    ///
    /// 不是 Miyu 的数据根；建不了目录、写不了标记，或者该是目录的地方是个文件。
    pub fn prepare(&self) -> Result<(), PrepareError> {
        create(&self.path)?;
        let marker = self.path.join(MARKER);
        if fs::symlink_metadata(&marker).is_err() {
            if fs::read_dir(&self.path)?.next().is_some() {
                return Err(PrepareError::NotOurs {
                    path: self.path.clone(),
                    old_miyu: looks_like_old_miyu(&self.path),
                });
            }
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&marker)?;
            file.write_all(MARKER_TEXT.as_bytes())?;
            file.sync_all()?;
        }
        for name in SKELETON {
            create(&self.path.join(name))?;
        }
        Ok(())
    }
}

/// 建骨架建不成。
#[derive(Debug)]
pub enum PrepareError {
    /// 目录里有别的东西，又没有标记：不是 Miyu 的数据根，一个字节都不动它。`old_miyu`：里面有
    /// 旧版 Miyu 特有的东西。
    NotOurs {
        /// 哪个目录。
        path: PathBuf,
        /// 看着像旧版 Miyu 的数据。
        old_miyu: bool,
    },
    /// 读写出错。
    Io(io::Error),
}

impl fmt::Display for PrepareError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PrepareError::NotOurs {
                path,
                old_miyu: true,
            } => write!(
                f,
                "{} 里像是旧版 Miyu 的数据，新版不动它。设 MIYU_HOME 指到别处，或者先迁过来",
                path.display()
            ),
            PrepareError::NotOurs {
                path,
                old_miyu: false,
            } => write!(
                f,
                "{} 里有别的东西，不像 Miyu 的数据根，新版不动它。设 MIYU_HOME 指到一个空目录",
                path.display()
            ),
            PrepareError::Io(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for PrepareError {}

impl From<io::Error> for PrepareError {
    fn from(error: io::Error) -> PrepareError {
        PrepareError::Io(error)
    }
}

/// 看着像旧版 Miyu 的数据：顶层有旧版特有的东西。只用来把报错说清楚，认不认得出都不碰它。
fn looks_like_old_miyu(path: &Path) -> bool {
    OLD_MIYU.iter().any(|name| path.join(name).exists())
}

/// 旧版 Miyu 的数据根顶层特有的几样（照旧版的代码和盘点，施工 3-1 补）。
const OLD_MIYU: [&str; 1] = ["config"];

/// 缓存目录：整台机器共用，不跟着 `MIYU_HOME` 变（`07-存储.md` 第二节）。只找，不建：用到它的
/// 到时候建。
///
/// # Errors
///
/// 要用家目录、`LOCALAPPDATA` 时找不到。
pub fn cache_root(env: &Env) -> Result<PathBuf, RootError> {
    Ok(match env.platform {
        Platform::Linux => match absolute(&env.xdg_cache_home) {
            Some(cache) => cache.join("miyu"),
            None => home(env)?.join(".cache").join("miyu"),
        },
        Platform::Macos => home(env)?.join("Library").join("Caches").join("Miyu"),
        Platform::Windows => local_app_data(env)?.join("Miyu").join("cache"),
    })
}

/// 设了、不是空的。
fn set(value: &Option<OsString>) -> Option<&OsString> {
    value.as_ref().filter(|value| !value.is_empty())
}

/// 设了、是绝对路径的：XDG 规范说相对路径的不算。
fn absolute(value: &Option<OsString>) -> Option<PathBuf> {
    set(value)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
}

/// 家目录，要是绝对路径。
fn home(env: &Env) -> Result<&Path, RootError> {
    env.home
        .as_deref()
        .filter(|home| home.is_absolute())
        .ok_or(RootError::NoHome)
}

/// `LOCALAPPDATA`，要是绝对路径。
fn local_app_data(env: &Env) -> Result<PathBuf, RootError> {
    absolute(&env.local_app_data).ok_or(RootError::NoLocalAppData)
}

/// 缺的才建，连同缺的上级目录：Unix 上权限 0700。已经有的不动。
pub(crate) fn create(path: &Path) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)
}

#[cfg(test)]
mod tests;
