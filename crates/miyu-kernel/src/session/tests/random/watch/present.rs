//! 回合开始交给挂接点的 `present`（施工 R-4 上）、`said`（施工 R-8）：看守照自己记下的日志重建有效历史（同 `compaction.rs`
//! 的 `rendered_order`），算出由模块注入的那几块、触发这一轮的人话，和内核交的一字不差。

use crate::facts::{Present, present, said};
use crate::history::History;
use crate::id::{Seq, TurnId};

use super::Watch;

impl Watch {
    /// 内核叫回合 `turn` 开始的挂接点：记下开头这一步（`manual.rs`），核对 `present`、`said`（`spoken`）和引用 `model`
    /// （`configure.rs`）。
    pub(super) fn turn_start_hooks(
        &mut self,
        turn: TurnId,
        given: &[Present],
        spoken: Option<&str>,
        model: Option<&str>,
    ) {
        self.start_hooks(turn);
        self.hooks_present(given, turn, spoken);
        self.hooks_model(model);
    }

    /// 内核叫挂接点（回合 `turn`）时带的 `present`、`said`（`spoken`），和看守照自己的日志算的对得上；有模块注入过的、人开
    /// 的一轮各记一条走到过的路。
    fn hooks_present(&mut self, given: &[Present], turn: TurnId, spoken: Option<&str>) {
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
        assert_eq!(
            spoken,
            said(&history, turn).as_deref(),
            "种子 {seed}：挂接点拿到的人话和有效历史对不上"
        );
        if spoken.is_some() {
            self.seen_paths.insert("挂接点拿到人话");
        }
    }
}
