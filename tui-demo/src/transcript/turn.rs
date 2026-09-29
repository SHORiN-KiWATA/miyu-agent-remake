//! 一轮开始、结束（蓝图 `tui.md`「正文」「时间线」）：开轮时把排队的消息挪进正文、记下这一轮；结束时停表、
//! 收起时间线、写收尾行，没收完的块算收全。

use super::*;

impl Transcript {
    /// 在等她的第一个字：一轮在跑、这一轮还一块都没来（出错等重试也算）。界面在正文末尾转圈，
    /// 步与步之间等她不算（蓝图 `tui.md`「时间线」第 19 条）。
    pub fn waiting(&self) -> bool {
        self.running.is_some() && !self.spoke
    }

    /// 她正在写的这一段正文：这一轮在跑，正文最后一条是这一轮的回答。长过视口时正文区停在它的开头
    /// （蓝图 `tui.md`「正文」第 1 条）。
    pub fn writing(&self) -> Option<&Entry> {
        self.running?;
        let last = self.entries.last()?;
        (last.kind == Kind::Reply && last.turn == self.turn).then_some(last)
    }

    /// 一轮开始了（`turn.started`）：开表、清这一轮的用量，把开这一轮的排队消息挪进正文。
    pub(super) fn start(&mut self, turn: u64, trigger: Option<u64>) {
        self.running = Some(Instant::now());
        self.spoke = false;
        self.failure = None;
        self.turn = Some(turn);
        self.turn_usage = Usage::default();
        self.turn_level = self.level;
        // 开这一轮的那一句：照 `trigger` 找序号对得上的；序号还没配上的，退回找还没归到哪一轮的最早那一句。
        let by_seq = self
            .entries
            .iter()
            .position(|e| trigger.is_some() && e.seq == trigger);
        let oldest = || {
            self.entries
                .iter()
                .position(|e| e.kind == Kind::User && e.turn.is_none())
        };
        let Some(at) = by_seq.or_else(oldest) else {
            return;
        };
        // 它和排在它前面、还排着的几条一起开这一轮：两下 Esc 打断以后排着的一起发，`trigger` 只指着其中一条
        // （`tui.md`「运行状态行和排队的消息」第 5 条）。排着的挪到正文末尾，按先后；本来不是排着的留在原位。
        let upto = self.entries[at].seq;
        let joins = |j: usize, e: &Entry| {
            j == at
                || (e.queued
                    && e.turn.is_none()
                    && match (e.seq, upto) {
                        (Some(a), Some(b)) => a <= b,
                        _ => j < at,
                    })
        };
        let picked: Vec<usize> = (0..self.entries.len())
            .filter(|&j| joins(j, &self.entries[j]))
            .collect();
        let mut moved = Vec::new();
        for &j in picked.iter().rev() {
            if self.entries[j].queued {
                moved.push(self.entries.remove(j));
            } else {
                self.entries[j].turn = Some(turn);
            }
        }
        // 几条一起发的拼成你说的一段话，按先后一条一行（`tui.md`「运行状态行和排队的消息」第 5 条）。
        let mut moved = moved.into_iter().rev();
        if let Some(mut first) = moved.next() {
            for next in moved {
                first.text.push('\n');
                first.text.push_str(&next.text);
                first.pasted.extend(next.pasted);
            }
            first.turn = Some(turn);
            first.queued = false;
            self.entries.push(first);
        }
    }

    /// 一轮结束了（`turn.ended`）：停表、收起时间线、写收尾行或打断、出错的那一行。
    pub(super) fn end(&mut self, reason: EndReason, texts: &Texts) {
        let took = self
            .running
            .take()
            .map_or(std::time::Duration::ZERO, |start| start.elapsed());
        self.retry = None;
        self.blocks.clear();
        self.unnamed.clear();
        self.finish_segment();
        self.settle_leftovers();
        let failure = self.failure.take();
        match reason {
            EndReason::Completed => {
                let (model, endpoint) = self
                    .model
                    .as_ref()
                    .map_or(("", ""), |(m, e)| (m.as_str(), e.as_str()));
                let time = jiff::Zoned::now().strftime("%H:%M").to_string();
                let text = texts
                    .done
                    .replace("{endpoint}", endpoint)
                    .replace("{model}", model)
                    .replace("{elapsed}", &crate::meter::seconds(took))
                    .replace("{time}", &time);
                let text = text + &words::turn_usage(&self.turn_usage, texts);
                self.push(Kind::Done, text);
            }
            // 和收尾行一个样子：`✻ 已中断`（`tui.md`「正文」第 4 条）。
            EndReason::Interrupted => self.push(Kind::Done, texts.interrupted.clone()),
            EndReason::Error => {
                let (class, message) = failure.unwrap_or_else(|| ("other".into(), String::new()));
                let text = texts
                    .failed
                    .replace("{class}", &class)
                    .replace("{message}", message.trim());
                self.push(Kind::Error, text);
            }
            EndReason::Other(other) => self.push(Kind::Note, other),
        }
        self.turn = None;
    }

    /// 这一轮结束了还没有结果的步骤（被打断的）：停表、不再转圈。
    pub(super) fn settle_leftovers(&mut self) {
        let segments = self.entries.iter_mut().filter_map(|e| e.segment.as_mut());
        for step in segments.flat_map(|s| s.steps.iter_mut()) {
            if step.busy() {
                step.stop();
                if let StepKind::Tool { state, .. } = &mut step.kind {
                    *state = ToolState::Done(ToolStatus::Cancelled);
                }
            }
        }
    }
}
