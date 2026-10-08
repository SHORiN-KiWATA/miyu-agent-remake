//! 扩展的标准错误（施工 9-4 上，`docs/blueprint/extensions.md`「怎么走」第 1 条）：直接接到 `state/logs/<编号>.stderr` 上，
//! 追加；核心不读它、不拷它。拉起前超过 [`LARGEST`] 的先挪成 `<编号>.stderr.old`（盖掉上一份）。停下了的，状态里给最后几行。

#[cfg(test)]
mod tests;

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use crate::Core;

/// 拉起前超过这么大的挪走：1 MiB。
pub(super) const LARGEST: u64 = 1 << 20;

/// 状态里给最后几行。
const LINES: usize = 20;

/// 状态里最多给多少字节：4 KiB。
const BYTES: u64 = 4096;

/// 包 `id` 的标准错误的文件。
pub(super) fn path(core: &Core, id: &str) -> PathBuf {
    core.root.state().join("logs").join(format!("{id}.stderr"))
}

/// 打开 `path` 接标准错误：目录没有的建，太大的先挪成 `.old`，追加。
pub(super) fn open(path: &Path) -> io::Result<File> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    if fs::metadata(path).is_ok_and(|meta| meta.len() > LARGEST) {
        let mut old = path.as_os_str().to_owned();
        old.push(".old");
        fs::rename(path, old)?;
    }
    OpenOptions::new().create(true).append(true).open(path)
}

/// `path` 最后 [`LINES`] 行，最多 [`BYTES`] 字节，从中间截的去掉截了半截的那一行；没有、空的、读不了的没有。
pub(super) fn tail(path: &Path) -> Option<String> {
    let mut file = File::open(path).ok()?;
    let length = file.metadata().ok()?.len();
    let start = length.saturating_sub(BYTES);
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    let whole = match start {
        0 => &text[..],
        _ => text.split_once('\n').map_or("", |(_, rest)| rest),
    };
    let lines: Vec<&str> = whole.lines().collect();
    let kept = lines[lines.len().saturating_sub(LINES)..].join("\n");
    (!kept.trim().is_empty()).then_some(kept)
}
