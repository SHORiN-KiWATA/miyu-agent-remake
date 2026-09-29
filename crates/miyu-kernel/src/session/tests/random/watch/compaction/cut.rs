//! 看守照规矩算的切法（`docs/blueprint/compaction.md` 第三条第 2 条）：切得开的地方、留尾巴。施工 6-8 从 `compaction.rs`
//! 挪出来。

use super::*;

/// 切在 `upto` 后面，前一段是不是投影的开头一段：序号不超过它的，都排在超过它的前面。
pub(super) fn cut_at(ordered: &[Event], upto: Seq) -> bool {
    let first = ordered.iter().position(|event| event.seq > upto);
    first.is_none_or(|first| ordered[first..].iter().all(|event| event.seq > upto))
}

/// 看守照规矩算的尾巴：预算是 `budget`（随机测试的策略：min(30, 压缩线的四分之一)，手动压缩、被动压缩没有压缩线的是
/// 30）。照投影的先后，一组从人的消息或者回复开始；从最新的一组往回，尾巴（这一组起到最后的全部事件）还在预算以内、
/// 这里切得开的，N 可以是这一组前面那一条，取最早的；加上就超了的停下。`keep_last`（被动压缩，施工 6-7）：最后一组比
/// 预算大也留。
pub(super) fn expected_tail(ordered: &[Event], budget: u64, keep_last: bool) -> Option<Seq> {
    let price = crate::estimate::Flat {
        image: 50,
        file: 50,
    };
    let mut tail = None;
    for (index, event) in ordered.iter().enumerate().rev() {
        if !matches!(event.body, Body::MessageUser(_) | Body::MessageAssistant(_)) {
            continue;
        }
        let size: u64 = ordered[index..]
            .iter()
            .map(|event| crate::estimate::event(event, &price))
            .sum();
        let lowest = ordered[index..]
            .iter()
            .map(|event| event.seq)
            .min()
            .unwrap();
        // 这一组前面切得开：前面的序号都比从这一组起的小。
        let cuttable = ordered[..index].iter().all(|event| event.seq < lowest);
        if size > budget {
            if keep_last && tail.is_none() && cuttable {
                tail = Some(seq(lowest.get() - 1));
            }
            break;
        }
        if cuttable {
            tail = Some(seq(lowest.get() - 1));
        }
    }
    tail
}
