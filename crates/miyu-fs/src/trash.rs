//! 系统的回收站（`10-自带软件.md` 第三节「`trash` 的细则」、第七节「改回文件的细则」）：放进去（施工 4-6 下，
//! `trash` 这件工具用），移回来（施工 4-7 上，撤销用）。施工 4-7 上从基础系统挪到这里：撤销照的是效果，第三方的
//! 编辑器报的 `file.trashed` 也要移得回来，这是碰文件的底子，不是哪个软件包的。
//!
//! 三个平台各做一份（`trash/` 下的 `linux.rs`、`macos.rs`、`windows.rs`）：Linux 照 freedesktop 的回收站规范自己放，
//! macOS 用系统的 `trashItemAtURL`，Windows 用 `trash` 这个 crate、再读回收站里的 `$I` 记录找回它（`recycled.rs`）。
//! 记下的位置三个平台都是回收站里的真实路径，移回来就是把它改名移回原处，再删掉回收站给它记的那一份（Linux 的
//! `.trashinfo`、Windows 的 `$I`）。

use std::io;
use std::path::Path;

#[cfg(target_os = "linux")]
#[path = "trash/linux.rs"]
mod bin;
#[cfg(target_os = "macos")]
#[path = "trash/macos.rs"]
mod bin;
#[cfg(windows)]
#[path = "trash/windows.rs"]
mod bin;
#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
#[path = "trash/other.rs"]
mod bin;
#[cfg_attr(
    not(windows),
    allow(
        dead_code,
        reason = "只有 Windows 用；每个平台都编它，它的测试到处都跑"
    )
)]
mod recycled;

/// 放不进回收站的几种。
#[derive(Debug)]
pub enum Refused {
    /// 这块盘上没有能放的回收站：没删。
    Unavailable,
    /// 挪了，可回收站里找不到它：它可能回不来了（只有 macOS、Windows 会碰到）。
    Lost,
    /// 出错了：系统说的原话。
    Failed(io::Error),
}

/// 把 `real` 放进回收站，交回它在回收站里的真实位置。`home` 是交给工具的家目录：Linux 的家目录回收站照它找。
///
/// # Errors
///
/// 这块盘上没有能放的回收站；挪了却找不到；系统报了错。
pub fn put(real: &Path, home: Option<&Path>) -> Result<String, Refused> {
    bin::put(real, home)
}

/// 把回收站里的 `kept`（[`put`] 交回的位置）移回 `to`：上级目录没了的建上，改名移回去，再删掉回收站给它记的那一份。
/// 记的那一份删不掉的，只记一条运行日志：东西已经回来了。
///
/// `to` 要空着，`kept` 要还在：先查的是调用的一方。查和移之间这一瞬 `to` 被别的程序占了的，改名会盖掉它，标准库
/// 没有「不盖」的改名（施工 4-7 上「风险」）。
///
/// # Errors
///
/// 上级目录建不了；改名移不回去（例如 `kept` 已经没了）。
pub fn restore(kept: &Path, to: &Path) -> io::Result<()> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::rename(kept, to)?;
    bin::forget(kept);
    Ok(())
}
