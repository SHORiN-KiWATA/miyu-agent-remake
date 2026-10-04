//! 这一轮出过几件事：运行状态行照它变没变换词（蓝图 `tui.md`「运行状态行和排队的消息」第 1 条）。

use super::{Kind, Transcript};

impl Transcript {
    /// 这一轮到现在出过几件事：开了几步、几步做完了（想完、工具出了结果）、开没开始写回答。
    /// 只增不减，变了就是出了新的事。
    pub fn beat(&self) -> usize {
        let Some(turn) = self.turn else {
            return 0;
        };
        self.entries
            .iter()
            .filter(|e| e.turn == Some(turn))
            .map(|e| match e.kind {
                Kind::Steps => e.segment.as_ref().map_or(0, |s| {
                    s.steps.len() + s.steps.iter().filter(|step| !step.busy()).count()
                }),
                Kind::Reply => 1,
                _ => 0,
            })
            .sum()
    }
}
