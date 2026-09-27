//! 效果（`docs/designs/10-自带软件.md` 第五节，施工 4-6 上）：工具报的效果带着改前改后的内容，这里存成 blob、
//! 换成哈希，写成内核的效果；照内核的效果记下她看过的文件，会话里记、载入时从日志里重建，走的是同一个函数。

use std::path::{Path, PathBuf};

use miyu_kernel::event::{Body, Effect, Event, FileChanged, FileRead, FileTrashed};
use miyu_kernel::id::ContentHash;
use miyu_store::blob::Blobs;
use miyu_tool::Seen;

use crate::TARGET;

/// 工具报的效果写成内核的：改前改后的内容存成 blob，换成它们的哈希。存不下来的（磁盘满了之类）照样记下哈希，
/// 写一条运行日志：撤销时发现 blob 没了，说改前的内容没存下来。碰磁盘，在阻塞线程里调。
pub(crate) fn store(blobs: &Blobs, effects: Vec<miyu_tool::Effect>) -> Vec<Effect> {
    effects
        .into_iter()
        .map(|effect| match effect {
            miyu_tool::Effect::Read { path, lines, hash } => Effect::FileRead(FileRead {
                path: text(&path),
                lines,
                hash,
            }),
            miyu_tool::Effect::Changed {
                path,
                before,
                after,
            } => Effect::FileChanged(FileChanged {
                path: text(&path),
                before: before.map(|content| put(blobs, &content)),
                after: put(blobs, &after),
            }),
            miyu_tool::Effect::Trashed { path, trash } => Effect::FileTrashed(FileTrashed {
                path: text(&path),
                trash,
            }),
        })
        .collect()
}

/// 照内核的效果记下她看过的文件：读过的记读的时候整份的哈希，写过的记改后的。移进了回收站的，不在了。
pub(crate) fn saw(seen: &mut Seen, effects: &[Effect]) {
    for effect in effects {
        match effect {
            Effect::FileRead(read) => {
                seen.insert(PathBuf::from(&read.path), read.hash.clone());
            }
            Effect::FileChanged(changed) => {
                seen.insert(PathBuf::from(&changed.path), changed.after.clone());
            }
            Effect::FileTrashed(trashed) => {
                seen.remove(Path::new(&trashed.path));
            }
            Effect::Unknown(_) => {}
        }
    }
}

/// 从日志里重建她看过的文件：照先后过一遍每一条工具结果的效果。
pub(crate) fn seen_in(events: &[Event]) -> Seen {
    let mut seen = Seen::new();
    for event in events {
        if let Body::ToolResult(result) = &event.body {
            saw(&mut seen, &result.effects);
        }
    }
    seen
}

/// 存一份内容，交回它的哈希。存不下来的照样算出哈希交回。
fn put(blobs: &Blobs, content: &[u8]) -> ContentHash {
    match blobs.put(content) {
        Ok(hash) => hash,
        Err(error) => {
            tracing::warn!(target: TARGET, error = %error, "effect content not stored");
            ContentHash::of(content)
        }
    }
}

/// 路径写成事件里的字。
fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests;
