//! 一个会话钉在一个 key 上（`docs/blueprint/models.md`「怎么走」第一条第 6 条、「起草时定的」第 3 条，施工 8-6）：会话编号的
//! SHA-256 前 8 个字节照大端当成一个无符号整数，对 key 的个数取余，就是它的 key。不用存，重启以后还是它；同一家的几个
//! key 上，会话大致均匀地分开，各自的缓存各自热。
//!
//! 候选的先后：会话的那一个在前，别的照写的先后跟在后面（第四条）。出错换 key 随 8-9。

use sha2::{Digest, Sha256};

/// 会话 `session`（编号的字）在 `count` 个 key 里钉着第几个，从 0 数。没有 key 的是空的。
pub fn pinned(session: &str, count: usize) -> Option<usize> {
    let count = u64::try_from(count).ok().filter(|count| *count > 0)?;
    let digest = Sha256::digest(session.as_bytes());
    let mut first = [0u8; 8];
    first.copy_from_slice(&digest[..8]);
    usize::try_from(u64::from_be_bytes(first) % count).ok()
}

/// 会话 `session` 用 `count` 个 key 时挑的先后：钉着的那一个在前，别的照写的先后。
pub fn order(session: &str, count: usize) -> Vec<usize> {
    let Some(first) = pinned(session, count) else {
        return Vec::new();
    };
    std::iter::once(first)
        .chain((0..count).filter(|at| *at != first))
        .collect()
}

#[cfg(test)]
mod tests;
