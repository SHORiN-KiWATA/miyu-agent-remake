//! 随机测试里的撤销、恢复和读回日志（施工 2-10、6-9）：各用一串随机数送，原来那串输入不跟着错开。

use super::super::revert::{revert, revert_last, unrevert};
use super::*;

/// 撤销、恢复，另用一串随机数：原来那串输入不跟着错开。空闲时四回里有一回，回合开着时五十回里
/// 一回（该被拒）。能恢复的时候一半是恢复，不能的时候十回里一回（该被拒）；撤销多半从还在有效历史
/// 里的最后三轮之一起，偶尔是对不上的。
pub(super) fn some_undo(rng: &mut Rng, watch: &Watch, next_id: &mut u64) -> Option<Input> {
    let chance = if watch.turn_open() { 50 } else { 4 };
    if rng.below(chance) != 0 {
        return None;
    }
    let n = next_command(next_id);
    let redo = match watch.undo.can_unrevert() {
        true => rng.below(2) == 0,
        false => rng.below(10) == 0,
    };
    if redo {
        return Some(unrevert(n));
    }
    // 四回里有一回不写回合编号，撤最后一轮（施工 4-7 下）。
    if rng.below(4) == 0 {
        return Some(revert_last(n));
    }
    // 三回里有一回撤到还算数的压缩所在的那一轮（施工 6-9）：压缩少，光从最后三轮里挑难得撤到它。
    let compacted = watch.compaction_turns();
    if !compacted.is_empty() && rng.below(3) == 0 {
        let turn = compacted[rng.below(compacted.len() as u64) as usize];
        return Some(revert(n, turn.started().get()));
    }
    let effective = &watch.undo.effective;
    let turn = match effective.len() {
        k if k > 0 && rng.below(6) > 0 => effective[k - 1 - rng.below(k.min(3) as u64) as usize]
            .started()
            .get(),
        _ => 1 + rng.below(watch.last()),
    };
    Some(revert(n, turn))
}

/// 读回日志的回报（施工 6-9）：在读回的时候，八回里有四回照日志回；一回少了最后一条、一回起点不对（都该不理）；两回
/// 送来一句话（该拒）。它排在每一步的最前面，撤销交出读回以后，这一步别的输入照常夹在中间，也有命令撞上读回。没在
/// 读回的时候八十回里一回送一个过时的（该不理）。
pub(super) fn some_read_back(rng: &mut Rng, watch: &Watch, next_id: &mut u64) -> Option<Input> {
    let Some((_, from)) = watch.undo.reading.clone() else {
        return (rng.below(80) == 0).then(|| read_back(seq(1), Vec::new()));
    };
    let mut events = watch.log_from(from);
    Some(match rng.below(8) {
        0 | 1 => send(next_command(next_id), "hi"),
        2 => {
            events.pop();
            read_back(from, events)
        }
        3 => read_back(from.next(), events.split_off(1)),
        _ => read_back(from, events),
    })
}

/// 在读回的话，照日志回：跑完三百条以后收尾用，撤销才有回应。
pub(super) fn read_back_now(watch: &Watch) -> Option<Input> {
    let (_, from) = watch.undo.reading.clone()?;
    Some(read_back(from, watch.log_from(from)))
}

/// 从第 `from` 条读回的这几条。
fn read_back(from: Seq, events: Vec<Event>) -> Input {
    Input::ReadBack {
        at: at(58),
        from,
        events,
    }
}
