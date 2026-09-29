//! 截短重试（`docs/blueprint/compaction.md` 第三条第 10 条，施工 6-6 中）：摘要请求自己报超长，截掉检查点后面最老的
//! 几组再发，最多截几次；截不动、截够了的，照一次压缩失败算。隔离式回退（第三条第 7 条、第四条，施工 6-6 下）：fork 式的
//! 摘要回复里调了工具，改发一次不带工具面的。两样都是同一次压缩换个样子再发一次摘要请求（[`Again`]）。
//!
//! 组和切法同留尾巴（`compaction.rs` 的 `cuts`）：一组从一条人的消息或者一条回复开始，只切在切得开的地方。按组不按轮：
//! 一轮任务里可以有几十组工具调用，按轮截，一轮的会话一组都截不掉。
//!
//! 出了事那一刻不当场重发：怎么再发记在回合上，回到准备好，这一批落了盘照常走发请求那一步（先落盘，后请求），发摘要
//! 请求时照它发（`compaction.rs` 的 `start_compaction`）。

use super::Session;
use super::compaction::{Compacting, Due, cuts};
use super::overflow::Passive;
use super::turn::Stage;
use crate::estimate;
use crate::event::{Body, CompactTrigger};
use crate::id::Seq;

/// 等着再发的那一次摘要请求：替代到哪、截到第几条（没截过的没有）、截着再试了几次、是不是隔离式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Again {
    pub(super) upto: Seq,
    pub(super) cut: Option<Seq>,
    pub(super) tries: u32,
    pub(super) isolated: bool,
}

impl Session {
    /// 摘要请求报了超长（`context_too_long`）：还能截的，记下截到哪，回到准备好，交回真；截不动、截够了次数、快照里没有
    /// 截短的数的，交回假，照失败算。`excess` 是驱动解析出的超了多少 token。
    pub(super) fn shorten(&mut self, compacting: &Compacting, excess: Option<u64>) -> bool {
        let Some(shorten) = self
            .policy
            .compaction
            .as_ref()
            .and_then(|compaction| compaction.shorten)
        else {
            return false;
        };
        let (cut, tries) = compacting.shortened();
        if tries >= shorten.tries {
            return false;
        }
        let Some(next) = self.next_cut(compacting.upto(), cut, excess, shorten.percent) else {
            return false;
        };
        self.again(
            Again {
                upto: compacting.upto(),
                cut: Some(next),
                tries: tries + 1,
                isolated: compacting.isolated(),
            },
            passive(compacting),
        )
    }

    /// fork 式的摘要回复里调了工具（第三条第 7 条，施工 6-6 下）：还没改走过、快照里有隔离式那句 system 的，记下改走
    /// 隔离式（截到哪照这一次的），回到准备好，交回真；别的交回假，照失败算。
    pub(super) fn isolate(&mut self, compacting: &Compacting) -> bool {
        if !self.can_isolate(compacting) {
            return false;
        }
        let (cut, tries) = compacting.shortened();
        self.again(
            Again {
                upto: compacting.upto(),
                cut,
                tries,
                isolated: true,
            },
            passive(compacting),
        )
    }

    /// 这一次调了工具的，能不能改走隔离式：还没改走过，快照里有那句 system。
    pub(super) fn can_isolate(&self, compacting: &Compacting) -> bool {
        !compacting.isolated()
            && self
                .policy
                .compaction
                .as_ref()
                .is_some_and(|compaction| compaction.isolate)
    }

    /// 记下怎么再发，回到准备好。被动压缩的（施工 6-7）连压什么也记回去：它不看压缩线，发请求那一步照它再压。
    fn again(&mut self, again: Again, due: Option<Due>) -> bool {
        let Some(turn) = self.turn.as_mut() else {
            return false;
        };
        turn.again = Some(again);
        turn.passive = due.map(Passive::Again);
        turn.stage = Stage::Ready;
        true
    }

    /// 再截到第几条：有效历史到第 `upto` 条的投影里，检查点后面、上一次截到的 `cut` 以后还剩的组，从最老的起去掉。有
    /// `excess` 的，去掉的估算加起来刚好盖过它（盖不过的，去到只剩一组）；没有的，去掉剩下的 `percent`%，向上取整、
    /// 至少一组。至少留一组：只剩一组的截不动，没有。
    fn next_cut(
        &self,
        upto: Seq,
        cut: Option<Seq>,
        excess: Option<u64>,
        percent: u32,
    ) -> Option<Seq> {
        let price = self.price()?;
        let history = self.history.until(upto);
        let ordered = history.ordered();
        let cuts = cuts(&ordered);
        // 上一次截掉的不算：投影的开头一段，序号都不超过它。
        let from = cut.map_or(0, |cut| {
            ordered
                .iter()
                .position(|event| event.seq > cut)
                .unwrap_or(ordered.len())
        });
        let starts: Vec<(usize, Seq)> = ordered
            .iter()
            .enumerate()
            .skip(from)
            .filter(|(_, event)| {
                matches!(event.body, Body::MessageUser(_) | Body::MessageAssistant(_))
            })
            .filter_map(|(index, _)| cuts[index].map(|seq| (index, seq)))
            .collect();
        let groups = starts.len();
        if groups <= 1 {
            return None;
        }
        let drop = match excess {
            Some(excess) => (1..groups)
                .find(|&k| {
                    let dropped: u64 = ordered[from..starts[k].0]
                        .iter()
                        .map(|event| estimate::event(event, &price))
                        .sum();
                    dropped >= excess
                })
                .unwrap_or(groups - 1),
            None => {
                let share = (groups * usize::try_from(percent).ok()?).div_ceil(100);
                share.clamp(1, groups - 1)
            }
        };
        Some(starts[drop].1)
    }
}

/// 被动压缩的（施工 6-7），再发时照它再压。
pub(super) fn passive(compacting: &Compacting) -> Option<Due> {
    (*compacting.trigger() == CompactTrigger::Overflow).then(|| compacting.due())
}

#[cfg(test)]
mod tests;
