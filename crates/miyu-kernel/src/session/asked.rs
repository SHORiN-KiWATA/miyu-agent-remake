//! 这一次是谁要她做的（施工 O-2 下，`docs/blueprint/providers.md`「是谁要的」）：回合记下她这时在回应的那一条的 `by`，派工具
//! 时带上（`tool.call` 的 `by`、`owner`）。开回合时是触发的那一条；之后每次请求前，请求新看到的排队消息、并进来的群消息
//! （`turn.joined` 的最后一条）、别的 harness 和别的会话发来的话里最新的那一条换上它。后台命令、子代理的回报是她自己派出去的
//! 事，不换。

use super::Session;
use super::jobs::Waker;
use crate::event::Body;
use crate::id::Seq;
use crate::origin::By;

impl Session {
    /// 第 `seq` 条的 `by`；不在手上的历史里的没有。
    pub(super) fn asker(&self, seq: Seq) -> Option<By> {
        self.history
            .events()
            .iter()
            .rev()
            .find(|event| event.seq == seq)
            .map(|event| event.by.clone())
    }

    /// 这次请求新看到的、有人说的话里最新的那一条的 `by`；没有的是没有，照旧。
    pub(super) fn newly_asked(&self) -> Option<By> {
        let turn = self.turn.as_ref()?;
        let queued = turn.queued.iter().map(|(seq, _)| *seq);
        let arrived = turn
            .reports
            .iter()
            .filter_map(|arrived| match arrived.waker {
                Waker::Job(_) => None,
                Waker::Joined => self.joined_last(arrived.seq),
                Waker::Harness | Waker::Peer => Some(arrived.seq),
            });
        queued.chain(arrived).max().and_then(|seq| self.asker(seq))
    }

    /// 第 `seq` 条 `turn.joined` 并进来的最后一条。
    fn joined_last(&self, seq: Seq) -> Option<Seq> {
        self.history
            .events()
            .iter()
            .rev()
            .find(|event| event.seq == seq)
            .and_then(|event| match &event.body {
                Body::TurnJoined(joined) => joined.triggers.last().copied(),
                _ => None,
            })
    }
}
