//! 替代到哪、切在哪（`docs/blueprint/compaction.md` 第三条第 2 条，施工 6-2 下）：留尾巴、切得开的地方、一组不拆。截短重试
//! （`shorten.rs`）也照这里的切法。

use crate::estimate::{self, Price};
use crate::event::{Body, Event};
use crate::id::Seq;

/// 留尾巴（`compaction.md` 第三条第 2 条，施工 6-2 下）：照投影的先后（`History::ordered`）看，一组从一条人的消息或者
/// 一条回复开始，两组之间的别的事件跟着前面那一组。从最新的一组往回，一组一组加进尾巴，加上就超过 `budget` 的停在它
/// 后面，交回尾巴前面那一条；最新的一组本身就超的，不留尾巴，是 `None`。
///
/// 只在「切得开」的地方切：前面的序号都比后面的小（[`cuts`]）。请求在路上时来的话，序号比回复小，投影里却排在回复后面，
/// 那里切不开，这一组并进前面那一组。`keep_last`（被动压缩，施工 6-7）：最新的一组本身就超的也留。
pub(super) fn tail_upto(
    ordered: &[&Event],
    budget: u64,
    price: &dyn Price,
    keep_last: bool,
) -> Option<Seq> {
    let cuts = cuts(ordered);
    let mut size = 0u64;
    let mut upto = None;
    for (index, event) in ordered.iter().enumerate().rev() {
        size = size.saturating_add(estimate::event(event, price));
        if !matches!(event.body, Body::MessageUser(_) | Body::MessageAssistant(_)) {
            continue;
        }
        if size > budget {
            // 最新的一组本身就超、切得开的，被动压缩照留（切不开的只会是请求在路上时排进来、还没人回应的话，替代到哪由
            // 「这一轮要回应的话」那条边界管，往前找也一样）。
            if keep_last
                && upto.is_none()
                && let Some(cut) = cuts[index]
            {
                upto = Some(cut);
            }
            break;
        }
        if let Some(cut) = cuts[index] {
            upto = Some(cut);
        }
    }
    upto
}

/// 投影里第 `i` 条前面能不能切：前面的序号都比从它起的小，能的交回切在哪（从它起最小的序号前面那一条）。
pub(in crate::session) fn cuts(ordered: &[&Event]) -> Vec<Option<Seq>> {
    let mut before = 0u64;
    let prefix: Vec<u64> = ordered
        .iter()
        .map(|event| {
            let max = before;
            before = before.max(event.seq.get());
            max
        })
        .collect();
    let mut after = u64::MAX;
    let mut cuts = vec![None; ordered.len()];
    for (index, event) in ordered.iter().enumerate().rev() {
        after = after.min(event.seq.get());
        if prefix[index] < after {
            cuts[index] = after.checked_sub(1).and_then(Seq::new);
        }
    }
    cuts
}

/// 定下切在哪（`compaction.md` 第三条第 2 条）：一组不拆，落在一条回复和它的工具结果之间的，退到这条回复前面；切出来的
/// 前一段要是投影的开头一段，不是的往前退到切得开的地方。两样都照到不动为止。
pub(super) fn settle(ordered: &[&Event], mut upto: Seq) -> Option<Seq> {
    loop {
        let split = ordered
            .iter()
            .filter_map(|event| match &event.body {
                Body::ToolResult(result) if event.seq > upto => Some(result.call_id.message()),
                _ => None,
            })
            .filter(|reply| *reply <= upto)
            .min();
        let mut next = match split {
            Some(reply) => Seq::new(reply.get().checked_sub(1)?)?,
            None => upto,
        };
        if let Some(first) = ordered.iter().position(|event| event.seq > next)
            && ordered[first..].iter().any(|event| event.seq <= next)
        {
            let lowest = ordered[first..].iter().map(|event| event.seq).min()?;
            next = Seq::new(lowest.get().checked_sub(1)?)?;
        }
        if next == upto {
            return Some(upto);
        }
        upto = next;
    }
}
