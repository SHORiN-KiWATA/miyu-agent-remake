//! 压完重读（`docs/blueprint/compaction.md` 第九条，施工 6-5）：内核交出要重读的文件，执行器在阻塞线程里一个一个读，
//! 读到的存成 blob，一个一项交回去。照「安全地打开」开：不跟最后一层的链接，不是普通文件的不读。检查点换了，内核要
//! 回原文，照 blob 读出来交回（施工 6-9）。

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use miyu_fs::open_file;
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

/// 取回原文（施工 6-9，`kernel/history.md`「重读的原文」）：内核要的检查点里重读过的文件，照 blob 读出来。读不出来的、
/// 不是 UTF-8 的不交，渲染时那一份整块不写。哪个检查点还算数是内核认的，执行器不自己找。
pub(crate) fn recall(wanted: &[ContentHash], blobs: &Blobs) -> BTreeMap<ContentHash, String> {
    wanted
        .iter()
        .filter_map(|blob| {
            let bytes = blobs.get(blob).ok()?;
            let text = String::from_utf8(bytes).ok()?;
            Some((blob.clone(), text))
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
