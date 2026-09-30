//! 环境快照：找数据根、资源目录要看的几样，从进程里读一次（`docs/designs/07-存储.md` 第二节「默认位置」
//! 「怎么找」，`12-进程形态与分发.md` 第三节「怎么找」）。
//!
//! 找数据根只照快照算，不直接读进程的环境：测试喂一份快照就行，不用改进程的环境变量（改了会串到
//! 同时跑的别的测试）。平台也是快照的一格，三个平台的默认位置在任何一台机器上都测得到。
//!
//! 系统的语言另读（[`locale`]，施工 8-1；环境变量都没设的看系统设置，施工 8-2）。

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

/// 平台：缓存目录的默认位置照它定；数据根三个平台都在家目录的 `.miyu` 里。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    /// Linux，和别的类 Unix 系统：缓存照 XDG。
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

/// 找数据根、资源目录要看的几样。没设的、读不到的是 `None`；空的、相对的算不算数，由用的地方定
/// （[`crate::root`]、[`crate::resources`]）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Env {
    /// 在哪个平台上。
    pub platform: Platform,
    /// `MIYU_HOME`：把数据根指到别处。
    pub miyu_home: Option<OsString>,
    /// 家目录：数据根在它下面的 `.miyu` 里。Windows 上是用户目录。
    pub home: Option<PathBuf>,
    /// `XDG_CACHE_HOME`（Linux）：缓存目录照它。
    pub xdg_cache_home: Option<OsString>,
    /// `LOCALAPPDATA`（Windows）：缓存目录照它。
    pub local_app_data: Option<OsString>,
    /// `MIYU_RESOURCES`：把资源目录指到别处，开发时指到源码树的 `resources/`（施工 3-6 上）。
    pub miyu_resources: Option<OsString>,
    /// 程序的真实位置：顺着链接找到的本体。资源目录在它旁边或者上一级（施工 3-6 上）。
    pub exe: Option<PathBuf>,
}

impl Env {
    /// 从进程里读一次。
    pub fn current() -> Env {
        Env {
            platform: Platform::current(),
            miyu_home: std::env::var_os("MIYU_HOME"),
            home: std::env::home_dir(),
            xdg_cache_home: std::env::var_os("XDG_CACHE_HOME"),
            local_app_data: std::env::var_os("LOCALAPPDATA"),
            miyu_resources: std::env::var_os("MIYU_RESOURCES"),
            exe: std::env::current_exe()
                .ok()
                .map(|exe| exe.canonicalize().unwrap_or(exe)),
        }
    }

    /// 环境变量里写的路径，开头是 `~` 的照家目录接上（施工 4-11：终端里 `export X=~/…` 加了引号，`~` 没被 shell
    /// 展开）：`~` 本身是家目录，`~/` 开头的接上后面，Windows 上 `~\` 也算；按一段段目录认，`~alice/…` 不认，照原样。
    /// 要接家目录、家目录却找不到（没有、不是绝对路径）的，是空的。
    pub fn expand(&self, value: &OsStr) -> Option<PathBuf> {
        let path = Path::new(value);
        let Ok(rest) = path.strip_prefix("~") else {
            return Some(path.to_path_buf());
        };
        let home = self.home.as_deref().filter(|home| home.is_absolute())?;
        Some(match rest.as_os_str().is_empty() {
            true => home.to_path_buf(),
            false => home.join(rest),
        })
    }
}

/// 看系统的语言要读的环境变量，照这个先后。
const LOCALE_VARS: [&str; 3] = ["LC_ALL", "LC_MESSAGES", "LANG"];

/// 系统的语言：`LC_ALL`、`LC_MESSAGES`、`LANG` 照这个先后，取第一个设了、不是空的（不是 UTF-8 的当没设），和命令行
/// 认界面语言的一样（`cli/main.md`「界面语言」）；这几个都没设的，照系统设置（[`system_locale`]：macOS 的首选语言、
/// Windows 的界面语言，施工 8-2）；都没有的是空的。核心要用语言、又没有头的时候照它，例如生成配置的 Schema 和参考
/// 文件（`config.md` 第一条第 6 条）；命令行握手以前照它挑界面语言、握手时报给核心（第二条第 8 条）。
pub fn locale() -> Option<String> {
    locale_from(|name| std::env::var(name).ok(), system_locale)
}

/// 系统设置里的语言，BCP 47 的写法（`zh-CN`、`ja-JP`）：`sys-locale` 读。Linux 上它也只看环境变量（多看一个
/// `LANGUAGE`），macOS 看首选语言，Windows 看用户的界面语言。读不出来、是空的，交回空的。
pub fn system_locale() -> Option<String> {
    sys_locale::get_locale().filter(|locale| !locale.is_empty())
}

/// 同 [`locale`]，环境变量照 `var` 读、系统设置照 `system` 读：测试喂一份，不改进程的环境。
fn locale_from(
    var: impl Fn(&str) -> Option<String>,
    system: impl FnOnce() -> Option<String>,
) -> Option<String> {
    LOCALE_VARS
        .iter()
        .find_map(|name| var(name).filter(|value| !value.is_empty()))
        .or_else(system)
}

#[cfg(test)]
mod tests;
