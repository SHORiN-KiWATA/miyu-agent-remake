//! 停着的（施工 4-9 再补一）：打断以后在等改文件的调用停下，多半很快交回来，一半停在改之前、一半改完了（带着
//! 效果）；偶尔到点，偶尔又打断一次；也有这一回不管、先来别的。

use super::*;

/// 在等停着的时候的下一条输入；没在等的，或者这一回不管，交回 `None`。
pub(super) fn some_stop_end(rng: &mut Rng, watch: &Watch, next_id: &mut u64) -> Option<Input> {
    let calls: Vec<CallId> = watch.stopping.calls.iter().copied().collect();
    if calls.is_empty() {
        return None;
    }
    let call_id = calls[rng.below(calls.len() as u64) as usize];
    let done = |stopped: bool, effects: Vec<crate::event::Effect>| Input::ToolDone {
        at: at(50),
        call_id,
        error: false,
        blocks: Vec::new(),
        duration_ms: Some(1),
        human: None,
        effects,
        stopped,
    };
    match rng.below(10) {
        0..=2 => Some(done(true, Vec::new())),
        3..=5 => Some(done(false, watch.some_effects())),
        6 => watch.stopping.wake.map(woke),
        7 => Some(some_interrupt(rng, next_id)),
        _ => None,
    }
}
