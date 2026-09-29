//! 套接字放哪（`docs/designs/07-存储.md` 第二节「怎么写」）：只照快照算，不碰磁盘。
//!
//! 一个数据根一个位置，名字里带数据根的指纹，几个数据根不撞：
//!
//! - Linux 上设了 `$XDG_RUNTIME_DIR` 的：`$XDG_RUNTIME_DIR/miyu-<指纹>/core.sock`；
//! - 没设的、别的平台：数据根的 `run/core.sock`；
//! - 路径太长放不下的：`$TMPDIR/miyu-<uid>/<指纹>.sock`；
//! - Windows：命名管道 `\\.\pipe\miyu-<指纹>`（施工 3-8 补）。

use std::path::{Path, PathBuf};

use miyu_store::env::Platform;
use miyu_store::root::DataRoot;
use sha2::{Digest, Sha256};

use crate::error::OpenError;

/// 找套接字放哪要看的几样，从进程里读一次。和 `miyu_store::env::Env` 一样只照快照算：测试喂一份
/// 快照，不改进程的环境变量。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dirs {
    /// 在哪个平台上：Linux 才用 `$XDG_RUNTIME_DIR`；路径的上限也照它。
    pub platform: Platform,
    /// `$XDG_RUNTIME_DIR`：读的时候核对过，是自己的、只有自己能进的目录；不是的当没设。
    pub runtime_dir: Option<PathBuf>,
    /// 临时目录：`$TMPDIR`，没设的是系统的默认（Linux 上是 `/tmp`）。
    pub temp_dir: PathBuf,
    /// 有效用户编号（Unix）：临时目录下的子目录照它起名。Windows 上没有。
    pub uid: Option<u32>,
}

impl Dirs {
    /// 从进程里读一次。
    pub fn current() -> Dirs {
        Dirs {
            platform: Platform::current(),
            runtime_dir: crate::sys::runtime_dir(std::env::var_os("XDG_RUNTIME_DIR")),
            temp_dir: std::env::temp_dir(),
            uid: crate::sys::uid(),
        }
    }
}

/// 套接字路径最多几个字节：Linux 的上限 108、macOS 的 104 都算上了结尾的零，所以各少一个。
/// Windows 上是命名管道，没有这个上限。
fn limit(platform: Platform) -> Option<usize> {
    match platform {
        Platform::Linux => Some(107),
        Platform::Macos => Some(103),
        Platform::Windows => None,
    }
}

/// 数据根的指纹：数据根路径的 SHA-256 前 8 位（十六进制）。照数据根写的路径算，不追链接：同一个
/// 数据根换个写法算出来不一样也不要紧，锁在数据根里，头照 `run/socket` 去连。
pub fn fingerprint(root: &DataRoot) -> String {
    let digest = Sha256::digest(root.path().as_os_str().as_encoded_bytes());
    digest[..4]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// 套接字放哪。
///
/// # Errors
///
/// 哪里都放不下。
pub(crate) fn locate(root: &DataRoot, dirs: &Dirs) -> Result<PathBuf, OpenError> {
    let print = fingerprint(root);
    let Some(limit) = limit(dirs.platform) else {
        return Ok(PathBuf::from(format!(r"\\.\pipe\miyu-{print}")));
    };
    let first = match (dirs.platform, &dirs.runtime_dir) {
        (Platform::Linux, Some(runtime)) => runtime.join(format!("miyu-{print}")).join("core.sock"),
        _ => root.run().join("core.sock"),
    };
    let fits = |path: &PathBuf| path.as_os_str().as_encoded_bytes().len() <= limit;
    if fits(&first) {
        return Ok(first);
    }
    let fallback = dirs.uid.map(|uid| {
        dirs.temp_dir
            .join(format!("miyu-{uid}"))
            .join(format!("{print}.sock"))
    });
    match fallback {
        Some(fallback) if fits(&fallback) => Ok(fallback),
        _ => Err(OpenError::TooLong(first)),
    }
}

/// 套接字所在的这一层是不是这个数据根专用的：放在 `$XDG_RUNTIME_DIR/miyu-<指纹>/` 里的是，交回这一层；
/// 核心走的时候连它一起删（施工 5-11 补）。数据根的 `run/`、临时目录下几个数据根共用的 `miyu-<uid>/`
/// 不是。
pub(crate) fn own_dir(path: &Path, dirs: &Dirs) -> Option<PathBuf> {
    let dir = path.parent()?;
    let runtime = dirs.runtime_dir.as_deref()?;
    (dirs.platform == Platform::Linux && dir.parent() == Some(runtime)).then(|| dir.to_path_buf())
}

// 测试里的路径是 Unix 的写法。平台是快照的一格，Windows 那一支在 Unix 上照样测得到。
#[cfg(all(test, unix))]
mod tests;
