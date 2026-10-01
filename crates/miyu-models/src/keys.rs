//! 一个会话钉在一个 key 上（`docs/blueprint/models.md`「怎么走」第一条第 6 条、「起草时定的」第 3 条，施工 8-6）：会话编号的
//! SHA-256 前 8 个字节照大端当成一个无符号整数，对 key 的个数取余，就是它的 key。不用存，重启以后还是它；同一家的几个
//! key 上，会话大致均匀地分开，各自的缓存各自热。
//!
//! 候选的先后：会话的那一个在前，别的照写的先后跟在后面（第四条）。出错换到别的 key、成了以后，这个会话一直用新的（施工
//! 8-9，路由记着，[`order`] 的 `first` 交进来）。
//!
//! key 的名字（[`name`]）：引用的写法 `secret:<名字>`、`env:<变量>`，`model.list` 的 `ref`、冷却的单位都用它（施工 8-9）。

use miyu_config::secret::Reference;
use sha2::{Digest, Sha256};

/// key 的名字：`secret:<名字>`、`env:<变量>`（施工 8-9 从 `model.list` 挪来，冷却也认它）。
pub fn name(reference: &Reference) -> String {
    match reference {
        Reference::Secret(name) => format!("secret:{name}"),
        Reference::Env(name) => format!("env:{name}"),
    }
}

/// 会话 `session`（编号的字）在 `count` 个 key 里钉着第几个，从 0 数。没有 key 的是空的。
pub fn pinned(session: &str, count: usize) -> Option<usize> {
    let count = u64::try_from(count).ok().filter(|count| *count > 0)?;
    let digest = Sha256::digest(session.as_bytes());
    let mut first = [0u8; 8];
    first.copy_from_slice(&digest[..8]);
    usize::try_from(u64::from_be_bytes(first) % count).ok()
}

/// 会话 `session` 用 `count` 个 key 时挑的先后：钉着的那一个在前，别的照写的先后。出错换过去、成了的那一个（`moved`，施工
/// 8-9）在的话它在前。
pub fn order(session: &str, count: usize, moved: Option<usize>) -> Vec<usize> {
    let Some(pinned) = pinned(session, count) else {
        return Vec::new();
    };
    let first = moved.filter(|at| *at < count).unwrap_or(pinned);
    std::iter::once(first)
        .chain((0..count).filter(|at| *at != first))
        .collect()
}

#[cfg(test)]
mod tests;
