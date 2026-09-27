//! 找不到时，同一个目录里相近的名字（Claude Code、opencode 都这么做，施工 4-4 下）：只差大小写的、主名一样
//! 扩展名不同的、改一两个字就一样的，最多 3 个。

use std::path::{Path, PathBuf};

/// 最多列几个。
const MOST: usize = 3;
/// 改几个字以内算相近。
const EDITS: usize = 2;
/// 名字短于这么多个字的，不照改几个字算：`a` 和 `b` 只差一个字，并不相近。
const SHORTEST: usize = 4;
/// 名字长于这么多个字的，不照改几个字算，省得一个目录里全是长名字时算得慢。
const LONGEST: usize = 128;

/// 真实的位置 `missing` 不存在：它所在的目录里，和它的名字相近的几个，越近的越前，一样近的照名字排。
pub(super) fn names(missing: &Path) -> Vec<PathBuf> {
    let (Some(dir), Some(name)) = (missing.parent(), missing.file_name()) else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let wanted = name.to_string_lossy().to_lowercase();
    let mut near: Vec<(usize, String)> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            closeness(&wanted, &name.to_lowercase()).map(|score| (score, name))
        })
        .collect();
    near.sort();
    near.into_iter()
        .take(MOST)
        .map(|(_, name)| dir.join(name))
        .collect()
}

/// 两个名字有多近，越小越近；不够近的是空的。两个都已经换成小写。
pub(super) fn closeness(wanted: &str, name: &str) -> Option<usize> {
    if wanted == name {
        return Some(0);
    }
    if stem(wanted) == stem(name) {
        return Some(1);
    }
    let short = wanted.chars().count().min(name.chars().count());
    let long = wanted.chars().count().max(name.chars().count());
    if short < SHORTEST || long > LONGEST {
        return None;
    }
    edits(wanted, name).map(|count| 1 + count)
}

/// 主名：最后一个 `.` 前面那一截；`.` 打头的（`.gitignore`）整个都是主名。
fn stem(name: &str) -> &str {
    match name.rfind('.') {
        Some(at) if at > 0 => &name[..at],
        _ => name,
    }
}

/// 从 `a` 改到 `b` 要改几个字（增、删、换各算一个），多于 [`EDITS`] 的是空的。
fn edits(a: &str, b: &str) -> Option<usize> {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.len().abs_diff(b.len()) > EDITS {
        return None;
    }
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let above = row[j + 1];
            row[j + 1] = (above + 1)
                .min(row[j] + 1)
                .min(diagonal + usize::from(ca != cb));
            diagonal = above;
        }
    }
    let count = row[b.len()];
    (count <= EDITS).then_some(count)
}
