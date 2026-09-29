//! 压完重读（`docs/blueprint/compaction.md` 第九条，施工 6-5）：内核交出要重读的文件，执行器在阻塞线程里一个一个读，
//! 读到的存成 blob，一个一项交回去。照「安全地打开」开：不跟最后一层的链接，不是普通文件的不读。

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use miyu_fs::open_file;
use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::ContentHash;
use miyu_kernel::session::Reread;
use miyu_store::blob::Blobs;

/// 照先后读 `paths`：超过 `limit` 字节的不读完，报太大；读到的存进 `blobs`。没有、读不了、不是普通文件、不是 UTF-8、
/// 存不进 blob 的，读不到。
pub(crate) fn reread(paths: &[String], limit: u64, blobs: &Blobs) -> Vec<Reread> {
    paths
        .iter()
        .map(|path| one(Path::new(path), limit, blobs))
        .collect()
}

/// 载入以后交回内核的原文（施工 6-5）：日志里最近一个检查点重读过的文件，照 blob 读出来。读不出来的、不是 UTF-8 的
/// 不交，渲染时那一份整块不写。
pub(crate) fn recalled(events: &[Event], blobs: &Blobs) -> BTreeMap<ContentHash, String> {
    let Some(restored) = events.iter().rev().find_map(|event| match &event.body {
        Body::ContextCompacted(compacted) => Some(&compacted.restored),
        _ => None,
    }) else {
        return BTreeMap::new();
    };
    restored
        .iter()
        .filter_map(|file| {
            let bytes = blobs.get(&file.blob).ok()?;
            let text = String::from_utf8(bytes).ok()?;
            Some((file.blob.clone(), text))
        })
        .collect()
}

fn one(path: &Path, limit: u64, blobs: &Blobs) -> Reread {
    let Ok(file) = open_file(path) else {
        return Reread::Unreadable;
    };
    // 多读一个字节：读满了上限还有剩的，就是太大。
    let mut bytes = Vec::new();
    if file
        .take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .is_err()
    {
        return Reread::Unreadable;
    }
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > limit {
        return Reread::TooLarge;
    }
    let Ok(text) = String::from_utf8(bytes) else {
        return Reread::Unreadable;
    };
    match blobs.put(text.as_bytes()) {
        Ok(blob) => Reread::Read { blob, text },
        Err(error) => {
            tracing::warn!(target: crate::TARGET, error = %error, "reread not stored");
            Reread::Unreadable
        }
    }
}

#[cfg(test)]
mod tests;
