//! Windows 的回收站（施工 4-6 下）：`trash` 这个 crate 删（系统的 `IFileOperation`，进回收站），删完在回收站里找到它，
//! 记下它在回收站里的真实位置：`$R` 开头的那个文件或者目录。
//!
//! 删之前看那块盘的根目录下有没有 `$Recycle.Bin`：网络盘、U 盘这类没有回收站的，那个 crate 会直接永久删掉还报成功。
//! 没有的当收不了。一块本地盘上从来没删过东西，也可能还没有它：宁可不删，不可删没了。
//!
//! 找的时候不照列表里的名字拼原来的路径：那是给人看的显示名，扩展名可能被藏了（[`recycled`]）。列表只用来筛出原来
//! 在同一个目录里的，再读每一个旁边的 `$I` 记录，照记录里完整的原路径对，删的时间最晚的那个是它。

use std::io;
use std::path::{Component, Path, PathBuf, Prefix};

use super::{Refused, recycled};

/// 把 `real` 放进回收站，交回它在回收站里的真实位置。
pub(super) fn put(real: &Path, _home: Option<&Path>) -> Result<String, Refused> {
    let plain = plain(real);
    let Some(root) = drive(&plain) else {
        return Err(Refused::Unavailable);
    };
    if !root.join("$Recycle.Bin").is_dir() {
        return Err(Refused::Unavailable);
    }
    trash::delete(&plain).map_err(|error| Refused::Failed(io::Error::other(error.to_string())))?;
    let parent = plain.parent().unwrap_or(&plain);
    let items = trash::os_limited::list().map_err(|_| Refused::Lost)?;
    items
        .iter()
        .filter(|item| same(&item.original_parent, parent))
        .filter_map(|item| {
            let kept = PathBuf::from(&item.id);
            let record = std::fs::read(recycled::record_of(&kept)?).ok()?;
            let recorded = recycled::recorded(&record)?;
            same(Path::new(&recorded.path), &plain).then_some((recorded.deleted, kept))
        })
        .max_by_key(|(deleted, _)| *deleted)
        .map(|(_, kept)| kept.to_string_lossy().into_owned())
        .ok_or(Refused::Lost)
}

/// 去掉 `\\?\` 这个前缀：`trash` 这个 crate 和回收站的记录都是普通的写法。
fn plain(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    match text.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with("UNC\\") => PathBuf::from(rest),
        _ => path.to_path_buf(),
    }
}

/// 盘符的根目录，例如 `C:\`；网络路径没有。
fn drive(path: &Path) -> Option<PathBuf> {
    match path.components().next()? {
        Component::Prefix(prefix) => match prefix.kind() {
            Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => {
                Some(PathBuf::from(format!("{}:\\", char::from(letter))))
            }
            _ => None,
        },
        _ => None,
    }
}

/// 两个路径是不是同一个：Windows 上不分大小写。
fn same(left: &Path, right: &Path) -> bool {
    left.to_string_lossy().to_lowercase() == right.to_string_lossy().to_lowercase()
}
