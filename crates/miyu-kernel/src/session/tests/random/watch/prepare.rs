//! 看守查提前压好（施工 6-11 上、下，`docs/blueprint/compaction.md` 第十五条）：
//!
//! - 旁路请求只在这一轮开着时发，一次至多一次在路上；请求是有效历史到 N 的清单加 `summarize`；
//! - 它的 `model.called`：不带回合编号、不带 `compaction`，`seen` 是 N，写没写成和替身送的算出来的一样；
//! - 换上：没有摘要请求就写下的压缩，替代到最近压好的那一份的 N，摘要是它的正文，`trigger` 照这一次的；交了重读的，后面
//!   不跟摘要请求；推的压好了带 `prepared`，当场压的不带；
//! - 到线时在路上的（施工 6-11 下）：等它，推的进度是它的、字数是替身送过的正文字数；等的时候什么都不请求；
//! - 载入以后全忘：内核只在内存里记着。

use super::*;
use crate::event::{CompactionDone, ContextCompacted, ModelCalled, Purpose};

/// 看守记着的提前压好。
#[derive(Debug, Default)]
pub(in super::super) struct Prepares {
    /// 这个种子开着提前压（`random/preparing.rs`）：载入以后照旧。
    pub(in super::super) seeded: bool,
    /// 这一轮开着：挂接点的结果交的。
    on: bool,
    /// 在路上的那一次。
    pub(in super::super) flight: Option<Flight>,
    /// 最近压好的那一份：照到第几条、正文。内核用不上扔掉的，这里不一定知道，只拿来对换上的。
    ready: Option<(Seq, String)>,
    /// 交了重读、正在换上的那一份。
    swapping: Option<(Seq, String)>,
    /// 摘要请求刚说完：紧跟着的压缩是当场压的。
    pub(super) summarized: bool,
    /// 刚换上：推的压好了带 `prepared`。
    swapped: bool,
    /// 回合在等在路上的那一次（施工 6-11 下）：推过它的进度，它还没说完、这一轮还没结束。
    awaiting: bool,
}

/// 在路上的那一次，和替身送过的回报。
#[derive(Debug)]
pub(in super::super) struct Flight {
    /// 照到第几条。
    pub(in super::super) upto: Seq,
    /// 报过发出去了。
    pub(in super::super) sent: bool,
    /// 正文那一块开始了。
    opened: bool,
    /// 送过的正文。
    text: String,
    /// 增量对不上了。
    broken: bool,
    /// 替身送了说完了：写成了的是那一份摘要。
    ended: Option<Option<String>>,
}

impl Flight {
    /// 下一段像样的增量：正文那一块还没开始的先开始，开始了的来一段字。
    pub(in super::super) fn next_delta(&self) -> Delta {
        match self.opened {
            false => Delta::Start {
                index: 0,
                kind: Kind::Text,
            },
            true => Delta::Text {
                index: 0,
                text: format!(" 摘要{}", self.upto),
            },
        }
    }

    /// 说过话了。
    pub(in super::super) fn said(&self) -> bool {
        !self.text.is_empty()
    }

    /// 替身送了一段增量：照内核的累积器算对不对得上。
    fn take(&mut self, delta: &Delta) {
        if self.broken {
            return;
        }
        match delta {
            _ if !self.sent => self.broken = true,
            Delta::Start {
                index: 0,
                kind: Kind::Text,
            } if !self.opened => self.opened = true,
            Delta::Text { index: 0, text } if self.opened => self.text.push_str(text),
            _ => self.broken = true,
        }
    }
}

impl Watch {
    /// 送进一条输入之前：挂接点的结果记下这一轮开没开；提前压的回报记在在路上的那一次上，对不上的不理。
    pub(super) fn before_prepare(&mut self, input: &Input) {
        match input {
            Input::TurnStartHooksDone { turn, prepare, .. }
                if self.turn_open() && self.open_turn() == *turn && !self.done.contains(turn) =>
            {
                self.prepares.on = *prepare;
            }
            Input::AsideSent {
                purpose: Purpose::Compaction,
                upto,
                ..
            }
            | Input::AsideDelta {
                purpose: Purpose::Compaction,
                upto,
                ..
            }
            | Input::AsideEnded {
                purpose: Purpose::Compaction,
                upto,
                ..
            } => {
                let Some(flight) = self.prepares.flight.as_mut().filter(|f| f.upto == *upto) else {
                    self.seen_paths.insert("对不上的提前压回报不理");
                    return;
                };
                match input {
                    Input::AsideSent { .. } => flight.sent = true,
                    Input::AsideDelta { delta, .. } => flight.take(delta),
                    Input::AsideEnded { error, .. } => {
                        let text = flight.text.trim().to_string();
                        let written = error.is_none() && !flight.broken && flight.sent;
                        flight.ended = Some((written && !text.is_empty()).then_some(text));
                        self.prepares.awaiting = false;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    /// 交出了一次提前压：这一轮开着，没有在路上的，请求照有效历史到 N 的清单加 `summarize`。
    pub(super) fn prepare_issued(&mut self, upto: Seq, request: &Request) {
        let seed = self.seed;
        self.seen_paths.insert("提前压了");
        assert!(self.prepares.on, "种子 {seed}：这一轮关着提前压，却交了");
        assert!(
            self.prepares.flight.is_none(),
            "种子 {seed}：一次至多一次提前压在路上"
        );
        let kept: Vec<Event> = self
            .effective_events()
            .into_iter()
            .filter(|event| event.seq <= upto)
            .collect();
        assert_eq!(
            listed_request(request),
            format!("{}summarize\n", listing(&kept)),
            "种子 {seed}：提前压照有效历史到第 {upto} 条"
        );
        self.prepares.ready = None;
        self.prepares.flight = Some(Flight {
            upto,
            sent: false,
            opened: false,
            text: String::new(),
            broken: false,
            ended: None,
        });
    }

    /// 提前压的 `model.called`：不带回合编号、不带 `compaction`，照到的是在路上的那一次，写没写成和算出来的一样。
    pub(super) fn prepare_called(&mut self, event: &Event, called: &ModelCalled) {
        let seed = self.seed;
        let flight = self
            .prepares
            .flight
            .take()
            .unwrap_or_else(|| panic!("种子 {seed}：没有在路上的提前压，却记了一条"));
        assert_eq!(event.turn, None, "种子 {seed}：提前压不带回合编号");
        assert_eq!(
            called.compaction, None,
            "种子 {seed}：提前压不带 compaction"
        );
        assert_eq!(called.seen, flight.upto, "种子 {seed}");
        match flight.ended.flatten() {
            Some(text) => {
                self.seen_paths.insert("提前压好了");
                assert_eq!(called.result, CallResult::Ok, "种子 {seed}");
                self.prepares.ready = Some((flight.upto, text));
            }
            None => {
                self.seen_paths.insert("提前压没成");
                assert_eq!(called.result, CallResult::Error, "种子 {seed}");
            }
        }
    }

    /// 一批动作查完了，交出的重读后面还没跟摘要请求：是换上提前压好的那一份，重读的是它的 N。
    pub(super) fn reread_unfollowed(&mut self) {
        let Some(seen) = self.compactions.reread.take() else {
            return;
        };
        let seed = self.seed;
        let ready = self.prepares.ready.take();
        assert!(
            ready.as_ref().is_some_and(|(upto, _)| *upto == seen),
            "种子 {seed}：重读后面没跟摘要请求，也不是换上压好的：{ready:?}"
        );
        self.prepares.swapping = ready;
    }

    /// 正在换上的那一份照到第几条（`watch/rebuild.rs`：重读的结果记在它上面）。
    pub(super) fn swapping(&self) -> Option<Seq> {
        self.prepares.swapping.as_ref().map(|(upto, _)| *upto)
    }

    /// 写下了一条压缩：紧跟着摘要请求的是当场压的，交回假；不是的是换上了提前压好的那一份，替代到它的 N、摘要是它的正文。
    pub(super) fn compaction_swapped(&mut self, compacted: &ContextCompacted) -> bool {
        let seed = self.seed;
        if std::mem::take(&mut self.prepares.summarized) {
            return false;
        }
        self.seen_paths.insert("换上了提前压好的");
        let (upto, text) = self
            .prepares
            .swapping
            .take()
            .or_else(|| self.prepares.ready.take())
            .unwrap_or_else(|| panic!("种子 {seed}：没有摘要请求、也没有压好的，却写了压缩"));
        assert_eq!(compacted.upto, upto, "种子 {seed}：换上的替代到它的 N");
        assert_eq!(compacted.summary, text, "种子 {seed}：换上的是它的摘要");
        assert_eq!(
            compacted.instructions, None,
            "种子 {seed}：附了要求的不换上"
        );
        self.prepares.swapped = true;
        true
    }

    /// 推了压好了：换上的带 `prepared`，当场压的不带。
    pub(super) fn done_prepared(&mut self, done: &CompactionDone) {
        let swapped = std::mem::take(&mut self.prepares.swapped);
        assert_eq!(done.prepared, swapped, "种子 {}", self.seed);
    }

    /// 这一轮结束了：正在换上的扔掉（打断、重启的收拾），不再等。
    pub(super) fn prepare_turn_ended(&mut self) {
        self.prepares.swapping = None;
        self.prepares.awaiting = false;
    }

    /// 推了进度，却没有在路上的摘要请求（施工 6-11 下）：是回合在等在路上的那一次提前压，进度是它的，字数是替身送过的
    /// 正文字数。
    pub(super) fn awaited_progress(&mut self, seen: Seq, written: u64) {
        let seed = self.seed;
        let flight = self
            .prepares
            .flight
            .as_ref()
            .filter(|flight| flight.upto == seen && flight.ended.is_none())
            .unwrap_or_else(|| {
                panic!("种子 {seed}：没有在路上的摘要请求、也没在等提前压，却推了进度")
            });
        let chars = u64::try_from(flight.text.chars().count()).unwrap_or(u64::MAX);
        assert_eq!(
            written, chars,
            "种子 {seed}：等的时候的字数是它收到的正文字数"
        );
        self.seen_paths.insert(match self.prepares.awaiting {
            false => "到线时等在路上的提前压",
            true => "等的时候推了它的进度",
        });
        self.prepares.awaiting = true;
    }

    /// 回合在等在路上的那一次提前压（施工 6-11 下，`random/preparing.rs` 的慢的照它说完）。
    pub(in super::super) fn awaiting(&self) -> bool {
        self.prepares.awaiting
    }

    /// 交出了请求（施工 6-11 下）：在等提前压的那一次时不该有。
    pub(super) fn not_awaiting(&self) {
        assert!(
            !self.prepares.awaiting,
            "种子 {}：等在路上的提前压时发了请求",
            self.seed
        );
    }

    /// 载入了：内核只在内存里记着，全忘。
    pub(super) fn forget_prepares(&mut self) {
        self.prepares = Prepares {
            seeded: self.prepares.seeded,
            ..Prepares::default()
        };
    }
}
