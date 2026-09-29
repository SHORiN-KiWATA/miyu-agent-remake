//! 回复每一块的起止（施工 2-3 补，`kernel/session.md`「收回复」第 2 条、第 3 条第 3 款）：照流里的块编号记下
//! 每一块第一段、最后一段增量到的时刻；收尾时照累积器交回的编号对上落盘的块，写进 `model.called` 的 `blocks`。

use crate::accumulate::Delta;
use crate::event::BlockSpan;
use crate::time::Timestamp;

/// 一次请求里，流里每一块的起止，照块的编号。
#[derive(Debug, Default)]
pub(super) struct Spans(Vec<(Timestamp, Timestamp)>);

/// 一段增量对起止的影响，在交给累积器之前取下：增量交出去了就拿不回来，收下了才照它记。
#[derive(Debug, Clone, Copy)]
pub(super) enum Mark {
    /// 一块开始了。
    Opens,
    /// 第几块来了字或者私有数据。
    Extends(usize),
    /// 收块的 `End`：不算。驱动流完了才一起收块（`drivers/openai-chat.md`「收尾」第 2 条），算上它，每一块都收在
    /// 流的末尾，思考了多久就算不出来。
    Closes,
}

impl Mark {
    /// 这一段增量的影响。
    pub(super) fn of(delta: &Delta) -> Mark {
        match delta {
            Delta::Start { .. } => Mark::Opens,
            Delta::Text { index, .. } | Delta::Private { index, .. } => Mark::Extends(*index),
            Delta::End { .. } => Mark::Closes,
        }
    }
}

impl Spans {
    /// 累积器收下了在 `at` 到的一段增量：开始的记下起止，字、私有数据把止挪到这一刻。只记收下了的：对不上的那一段，
    /// 这次请求按出错收，不算进哪一块。时钟往回拨了，止不往回挪，不早于起。
    pub(super) fn mark(&mut self, at: Timestamp, mark: Mark) {
        match mark {
            Mark::Opens => self.0.push((at, at)),
            Mark::Extends(index) => {
                if let Some((_, end)) = self.0.get_mut(index) {
                    *end = (*end).max(at);
                }
            }
            Mark::Closes => {}
        }
    }

    /// 落盘的那几块的起止，照它们在流里的编号、落盘的先后，从请求发出去的 `sent` 算起。累积器收下的每一块都记过
    /// 开始，编号都取得到。
    pub(super) fn of(
        &self,
        sent: Timestamp,
        kept: impl IntoIterator<Item = usize>,
    ) -> Vec<BlockSpan> {
        kept.into_iter()
            .filter_map(|index| self.0.get(index))
            .map(|(start, end)| BlockSpan {
                start_ms: millis(sent, *start),
                end_ms: millis(sent, *end),
            })
            .collect()
    }
}

/// 从 `from` 到 `to` 过了多少毫秒。时钟往回拨了，算 0。
pub(super) fn millis(from: Timestamp, to: Timestamp) -> u64 {
    u64::try_from(to.unix_millis().saturating_sub(from.unix_millis())).unwrap_or(0)
}
