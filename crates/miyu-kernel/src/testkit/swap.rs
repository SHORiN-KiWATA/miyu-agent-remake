//! 换策略快照（施工 P-1 再补，`docs/blueprint/kernel/session.md`「换策略快照」）：下一个回合开始时换上另一份策略，像执行器
//! 看到人格的文件改了那样。

use super::Stage;
use crate::id::ContentHash;
use crate::session::Policy;

impl Stage {
    /// 下一个回合开始时换上照 `policy` 造的那一份，新快照的哈希是 `hash`。之后重启、崩了再载入也照它造。
    ///
    /// # Panics
    ///
    /// `hash` 不合内容哈希的写法。
    pub fn swap_policy(&mut self, hash: &str, policy: impl Fn() -> Policy + 'static) {
        let hash = ContentHash::parse(hash).unwrap_or_else(|e| panic!("快照哈希的写法坏了：{e}"));
        self.swap = Some((hash, Box::new(policy)));
    }

    /// 回合开始的挂接点：排着换的，先交给内核放着，交回它的哈希；造策略的办法换成它的。
    pub(super) fn swapped(&mut self) -> Option<ContentHash> {
        let (hash, make) = self.swap.take()?;
        self.session.stage_policy(make());
        self.policy = make;
        Some(hash)
    }
}
