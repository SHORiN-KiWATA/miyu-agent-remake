//! 验过的登录令牌（`onebot.md` 第二条「施工时定的」第 3 条）：页面每 5 秒取一次 `/status`，验过的记一阵，这一阵里不再和核心
//! 握手。只记令牌的 SHA-256：桥的内存里不留登录令牌的原文（照核心存登录令牌的规矩，`web-module.md` 第一条第 6 条）。
//! 只记验得过的；过了时候的下一次问时扔掉。

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

/// 验过的：令牌的哈希 → 什么时候验的。
pub(crate) struct Checked {
    /// 记多久。
    keep: Duration,
    seen: Mutex<HashMap<String, Instant>>,
}

impl Checked {
    /// 验过的记 `keep` 这么久。
    pub(crate) fn new(keep: Duration) -> Checked {
        Checked {
            keep,
            seen: Mutex::new(HashMap::new()),
        }
    }

    /// `token` 在 `now` 以前 `keep` 之内验过。顺手扔掉过了时候的。
    pub(crate) fn fresh(&self, token: &str, now: Instant) -> bool {
        let mut seen = self.seen();
        seen.retain(|_, checked| now.saturating_duration_since(*checked) < self.keep);
        seen.contains_key(&hash(token))
    }

    /// `token` 在 `now` 验过了。
    pub(crate) fn remember(&self, token: &str, now: Instant) {
        self.seen().insert(hash(token), now);
    }

    /// 锁里不 `await`、不会崩；真崩了，表照样能用。
    fn seen(&self) -> std::sync::MutexGuard<'_, HashMap<String, Instant>> {
        self.seen.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// 记着的键（测试看里面没有原文）。
    #[cfg(test)]
    pub(crate) fn keys(&self) -> Vec<String> {
        self.seen().keys().cloned().collect()
    }
}

/// 令牌的 SHA-256，64 位小写十六进制。
pub(crate) fn hash(token: &str) -> String {
    Sha256::digest(token.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
