//! 环境快照：找数据根要看的几样，从进程里读一次（`docs/designs/07-存储.md` 第二节「默认位置」
//! 「怎么找」）。
//!
//! 找数据根只照快照算，不直接读进程的环境：测试喂一份快照就行，不用改进程的环境变量（改了会串到
//! 同时跑的别的测试）。平台也是快照的一格，三个平台的默认位置在任何一台机器上都测得到。

use std::ffi::OsString;
use std::path::PathBuf;

/// 平台：数据根、缓存目录的默认位置照它定。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    /// Linux，和别的类 Unix 系统：照 XDG。
    Linux,
    /// macOS。
    Macos,
    /// Windows。
    Windows,
}

impl Platform {
    /// 编译的目标平台。别的类 Unix 系统照 Linux 的走。
    pub fn current() -> Platform {
        if cfg!(target_os = "macos") {
            Platform::Macos
        } else if cfg!(windows) {
            Platform::Windows
        } else {
            Platform::Linux
        }
    }
}

/// 找数据根要看的几样。没设的、读不到的是 `None`；空的、相对的算不算数，由用的地方定
/// （[`crate::root`]）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Env {
    /// 在哪个平台上。
    pub platform: Platform,
    /// `MIYU_HOME`：把数据根指到别处。
    pub miyu_home: Option<OsString>,
    /// 家目录。
    pub home: Option<PathBuf>,
    /// `XDG_DATA_HOME`（Linux）。
    pub xdg_data_home: Option<OsString>,
    /// `XDG_CACHE_HOME`（Linux）。
    pub xdg_cache_home: Option<OsString>,
    /// `LOCALAPPDATA`（Windows）。
    pub local_app_data: Option<OsString>,
}

impl Env {
    /// 从进程里读一次。
    pub fn current() -> Env {
        Env {
            platform: Platform::current(),
            miyu_home: std::env::var_os("MIYU_HOME"),
            home: std::env::home_dir(),
            xdg_data_home: std::env::var_os("XDG_DATA_HOME"),
            xdg_cache_home: std::env::var_os("XDG_CACHE_HOME"),
            local_app_data: std::env::var_os("LOCALAPPDATA"),
        }
    }
}
