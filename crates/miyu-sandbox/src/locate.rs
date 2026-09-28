//! 找助手（`docs/blueprint/sandbox.md`「怎么走」第 1 条）：主程序真实位置旁边的 `miyu-sandbox`。

use std::path::{Path, PathBuf};

/// 助手的文件名：Windows 上带 `.exe`。
pub const HELPER: &str = if cfg!(windows) {
    "miyu-sandbox.exe"
} else {
    "miyu-sandbox"
};

/// 主程序的真实位置是 `exe`，旁边的助手。没有的、不是普通文件的，是空的。
pub fn locate(exe: &Path) -> Option<PathBuf> {
    let helper = exe.parent()?.join(HELPER);
    helper.is_file().then_some(helper)
}
