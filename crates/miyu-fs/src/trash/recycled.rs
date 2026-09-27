//! Windows 回收站里的 `$I` 记录（施工 4-6 下）：系统把删掉的东西改名成 `$R` 开头的，旁边放一份名字一样、`$I` 开头的
//! 记录，写着原来的完整路径和删的时间。回收站列表里的名字是给人看的显示名，「隐藏已知文件类型的扩展名」开着的时候
//! （Windows 默认开着）没有扩展名，`a.txt` 列成 `a`，拼不回原来的路径（施工 4-6 下在 CI 的 Windows 上查实的），所以
//! 要读这份记录。
//!
//! 格式：头 8 字节是版本，接着是大小、删的时间各 8 字节。第 2 版（Windows 10 起）再是 4 字节的字数（连结尾的 0）和
//! UTF-16 的路径；第 1 版（更早的）路径定长 260 个字，不够的补 0。
//!
//! 只有 Windows 用它；每个平台都编它，它的测试到处都跑。

use std::path::{Path, PathBuf};

#[cfg(test)]
mod tests;

/// 一份 `$I` 记录里写的。
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Recorded {
    /// 原来的完整路径。
    pub(super) path: String,
    /// 删的时间：Windows 的 FILETIME，1601 年起的 100 纳秒数。比回收站列表里精确到秒的时间细，一秒里删了两次也
    /// 分得清哪个是后删的。
    pub(super) deleted: u64,
}

/// `$R` 开头的那个东西旁边那份 `$I` 记录在哪：名字的头两个字换成 `$I`。不是 `$R` 开头的没有。
pub(super) fn record_of(kept: &Path) -> Option<PathBuf> {
    let name = kept.file_name()?.to_str()?;
    let rest = name.strip_prefix("$R")?;
    Some(kept.with_file_name(format!("$I{rest}")))
}

/// 读一份记录。认不出的版本、不够长的没有。
pub(super) fn recorded(record: &[u8]) -> Option<Recorded> {
    let number = |at: usize| {
        let bytes = record.get(at..at.checked_add(8)?)?;
        Some(u64::from_le_bytes(bytes.try_into().ok()?))
    };
    let wide = match number(0)? {
        1 => record.get(24..24 + 520)?,
        2 => {
            let count = u32::from_le_bytes(record.get(24..28)?.try_into().ok()?);
            let end = usize::try_from(count)
                .ok()?
                .checked_mul(2)?
                .checked_add(28)?;
            record.get(28..end)?
        }
        _ => return None,
    };
    let units: Vec<u16> = wide
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .take_while(|&unit| unit != 0)
        .collect();
    Some(Recorded {
        path: String::from_utf16_lossy(&units),
        deleted: number(16)?,
    })
}
