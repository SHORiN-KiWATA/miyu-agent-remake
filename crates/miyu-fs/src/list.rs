//! 列一层目录（`fs.list`，施工 W-2，`docs/blueprint/web-module.md`「三、列文件、找文件」第 3 条）：像 shell
//! 补全那样只读这一层，不往下走。

use std::fs;
use std::path::{Path, PathBuf};

use crate::boundary::{Boundary, Zone};
use crate::find::SHOWN;

/// 列目录的一条。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// 列表上写的名字：目录带 `/`。
    pub path: String,
    /// 真实的位置：`dir` 接上这一条的名字，不额外跟链接。
    pub full: PathBuf,
    /// 是不是目录。
    pub dir: bool,
}

/// 列 `dir` 这一层：名字照 `prefix` 开头对、大小写不论；点开头的要 `prefix` 也以 `.` 开头才列；目录在前、文件在后，
/// 各照名字排（大小写不论）；落进边界表「谁都不能碰」那一片的不列。交回最多 [`SHOWN`] 条，和有没有列全。
///
/// # Errors
///
/// 读不了 `dir`：不存在、不是目录、没有权限。
pub fn list_dir(
    dir: &Path,
    prefix: &str,
    boundary: &Boundary,
) -> std::io::Result<(Vec<Entry>, bool)> {
    let hidden_ok = prefix.starts_with('.');
    let want = prefix.to_lowercase();
    let mut items: Vec<(bool, String, PathBuf)> = fs::read_dir(dir)?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') && !hidden_ok {
                return None;
            }
            if !name.to_lowercase().starts_with(&want) {
                return None;
            }
            let full = entry.path();
            if boundary.zone(&full) == Zone::Forbidden {
                return None;
            }
            let is_dir = entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false);
            Some((is_dir, name, full))
        })
        .collect();
    items.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| a.1.to_lowercase().cmp(&b.1.to_lowercase()))
    });
    let partial = items.len() > SHOWN;
    items.truncate(SHOWN);
    let entries = items
        .into_iter()
        .map(|(is_dir, name, full)| Entry {
            path: if is_dir { format!("{name}/") } else { name },
            full,
            dir: is_dir,
        })
        .collect();
    Ok((entries, partial))
}

#[cfg(test)]
mod tests;
