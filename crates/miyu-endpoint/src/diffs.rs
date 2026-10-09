//! 两份内容之间的差异（施工 4-7 再补，`docs/blueprint/protocol/undo.md` 第 8 条；施工 9-6 三补从 `undo.rs` 抽出来，
//! `view.detail` 共用）：统一格式，上下文 3 行，每段 `@@ … @@` 那一行起头，接着是 ` `、`-`、`+` 开头的行；不带 `---`、`+++`
//! 那两行，文件结尾没有换行的也不加「没有换行」那一句。一边超过 1 MiB、不是 UTF-8、取不出来的算不出，说是哪一种。

use similar::TextDiff;

use miyu_kernel::id::ContentHash;
use miyu_store::blob::Blobs;

/// 一边最多多少字节：1 MiB。
pub(crate) const MOST_BYTES: u64 = 1 << 20;

/// 算不出差异的原因（`view.detail` 的 `skipped`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Skipped {
    /// 一边超过 [`MOST_BYTES`]。
    TooBig,
    /// 一边不是 UTF-8。
    Binary,
    /// 一边取不出来。
    Missing,
}

impl Skipped {
    /// 协议上的写法。
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Skipped::TooBig => "too_big",
            Skipped::Binary => "binary",
            Skipped::Missing => "missing",
        }
    }
}

/// 整份差异：几行，照整份数的新增、删掉的行数。两边一样的 `lines` 是空的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Lines {
    pub(crate) lines: Vec<String>,
    pub(crate) added: usize,
    pub(crate) removed: usize,
}

/// blob `hash` 的内容：没有 `hash`（`None`，新建的文件的改前）的照空的算。
///
/// # Errors
///
/// 取不出来、超过 [`MOST_BYTES`]。
pub(crate) fn side(blobs: &Blobs, hash: Option<&ContentHash>) -> Result<Vec<u8>, Skipped> {
    let bytes = match hash {
        None => Vec::new(),
        Some(hash) => blobs.get(hash).map_err(|_| Skipped::Missing)?,
    };
    match u64::try_from(bytes.len()).unwrap_or(u64::MAX) <= MOST_BYTES {
        true => Ok(bytes),
        false => Err(Skipped::TooBig),
    }
}

/// `then` 和 `now` 之间的整份差异。
///
/// # Errors
///
/// 一边不是 UTF-8。
pub(crate) fn unified(then: &[u8], now: &[u8]) -> Result<Lines, Skipped> {
    let (Ok(then), Ok(now)) = (std::str::from_utf8(then), std::str::from_utf8(now)) else {
        return Err(Skipped::Binary);
    };
    let text = TextDiff::from_lines(then, now)
        .unified_diff()
        .context_radius(3)
        .missing_newline_hint(false)
        .to_string();
    let lines: Vec<String> = text.lines().map(str::to_string).collect();
    let added = lines.iter().filter(|line| line.starts_with('+')).count();
    let removed = lines.iter().filter(|line| line.starts_with('-')).count();
    Ok(Lines {
        lines,
        added,
        removed,
    })
}

#[cfg(test)]
mod tests;
