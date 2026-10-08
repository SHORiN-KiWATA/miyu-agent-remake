//! 回合开始交给挂接点的 `present`（施工 R-4 上）：看守照自己记下的日志重建有效历史（同 `compaction.rs` 的
//! `rendered_order`），算出由模块注入的那几块，和内核交的一字不差。

use crate::facts::{Present, present};
use crate::history::History;
use crate::id::Seq;

use super::Watch;

impl Watch {
    /// 内核叫挂接点时带的 `present`，和看守照自己的日志算的对得上；有模块注入过的记一条走到过的路。
    pub(super) fn hooks_present(&mut self, given: &[Present]) {
        let seed = self.seed;
        let from = self.compactions.upto().map_or(Seq::FIRST, Seq::next);
        let mut history = History::whole();
        for event in std::iter::once(self.created()).chain(self.events.iter().cloned()) {
            if event.seq >= from {
                history.append(event);
            }
        }
        history.settle();
        assert_eq!(
            given,
            present(&history).as_slice(),
            "种子 {seed}：挂接点拿到的 present 和有效历史对不上"
        );
        if !given.is_empty() {
            self.seen_paths.insert("挂接点看得到模块注入过的");
        }
    }
}
