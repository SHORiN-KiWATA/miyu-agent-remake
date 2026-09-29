//! 看守查截短重试（`docs/blueprint/compaction.md` 第三条第 10 条，施工 6-6 中）：
//!
//! - 摘要请求报超长、这一批到它的 `model.called` 为止的，是截短了等着再发：不交到点叫醒；
//! - 截过的摘要请求只跟在报了超长的那一次后面，替代到的一样；截到的一次比一次晚，最多截 3 次；请求是截到的以后、
//!   第 N 条以前的清单，最前面写着截到哪；
//! - 截过的压缩，代码写的几段最后是摘要没看到的那一段：从上一个检查点后面第一条到截到的那一条；没截过的没有。

use super::*;
use crate::event::ContextCompacted;

/// 随机测试的截短的数（`random/compacting.rs`）。
const TRIES: u32 = 3;

/// 看守记着的截短。
#[derive(Default)]
pub(super) struct Shortenings {
    /// 报了超长、截短了等着再发的那一次摘要请求：替代到哪，那一次截到哪、截了几次。
    pending: Option<(Seq, Option<Seq>, u32)>,
    /// 最近发的那一次摘要请求：替代到哪、截到哪、截了几次。
    current: Option<(Seq, Option<Seq>, u32)>,
}

impl Watch {
    /// 替身的组装写在截过的摘要请求最前面的那一行里的：截到第几条。
    pub(super) fn truncated(request: &Request) -> Option<Seq> {
        let first = listed_request(request);
        let line = first.lines().next()?;
        let cut = line.strip_prefix("truncated after ")?;
        Seq::new(cut.parse().ok()?)
    }

    /// 摘要请求报了超长，这一批到它为止：截短了，等着再发。
    pub(super) fn summary_too_long(&mut self, seen: Seq) {
        let seed = self.seed;
        self.seen_paths.insert("摘要请求超长截短");
        let (cut, tries) = match self.shortenings.current {
            Some((upto, cut, tries)) if upto == seen => (cut, tries),
            _ => (None, 0),
        };
        assert!(tries < TRIES, "种子 {seed}：截够了 {TRIES} 次还在截");
        self.shortenings.pending = Some((seen, cut, tries));
    }

    /// 发了一次摘要请求：截过的，要跟在报了超长的那一次后面、截到的比上一次晚。交回截到哪。
    pub(super) fn summary_cut(&mut self, seen: Seq, request: &Request) -> Option<Seq> {
        let seed = self.seed;
        let cut = Watch::truncated(request);
        let pending = self.shortenings.pending.take();
        let tries = match (cut, pending) {
            (Some(cut), Some((upto, before, tries))) if upto == seen => {
                self.seen_paths.insert("截短了再发");
                assert!(
                    before.is_none_or(|before| cut > before),
                    "种子 {seed}：截到的没比上一次晚"
                );
                assert!(cut < seen, "种子 {seed}：截过了替代到的那一条");
                tries + 1
            }
            (Some(_), _) => panic!("种子 {seed}：没报超长的摘要请求却截了"),
            (None, Some((upto, _, _))) => {
                assert_ne!(upto, seen, "种子 {seed}：报了超长，再发却没截");
                0
            }
            (None, None) => 0,
        };
        self.shortenings.current = Some((seen, cut, tries));
        cut
    }

    /// 写了压缩：截过的，代码写的几段最后是摘要没看到的那一段；没截过的没有。
    pub(super) fn shorten_compacted(&mut self, compacted: &ContextCompacted) {
        let seed = self.seed;
        let cut = match self.shortenings.current {
            Some((upto, cut, _)) if upto == compacted.upto => cut,
            _ => None,
        };
        match cut {
            Some(cut) => {
                self.seen_paths.insert("截过的压缩写明没看到的");
                let from = self.compactions.upto.map_or(1, |upto| upto.get() + 1);
                assert!(
                    compacted
                        .notes
                        .ends_with(&format!("<uncovered {from}-{cut}/>")),
                    "种子 {seed}：截过的压缩，最后写摘要没看到的 {from}-{cut}：{:?}",
                    compacted.notes
                );
            }
            None => assert!(
                !compacted.notes.contains("<uncovered"),
                "种子 {seed}：没截过的写了摘要没看到的那一段"
            ),
        }
    }

    /// 回合结束了：等着再发的作废。
    pub(super) fn shorten_turn_ended(&mut self) {
        self.shortenings.pending = None;
    }
}
