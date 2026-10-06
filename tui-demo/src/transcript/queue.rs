//! 排着队的话（蓝图 `tui.md`「运行状态行和排队的消息」第 5 条）：回答进行中发的话先排着，记在正文末尾但不画。
//! 核心下一次请求就带上它（`kernel/session.md`「排队的消息」第 1 条）：哪一次请求推来的字里 `seen` 够着它的
//! 序号，它就被听到了，前面那段时间线收起，它进正文，接着的步另起一段。没被听到、这一轮就结束了的，由下一轮的
//! `turn.started` 带进来（`turn.rs`）。

use super::{Chip, Entry, Kind, Slot, StepKind, Transcript};

impl Transcript {
    /// 正文里最后一条不是排着队的：在进行的那一段、在写的回答照它找。排着的话没进正文，不算。
    pub(super) fn tail(&self) -> Option<usize> {
        self.entries.iter().rposition(|e| !e.queued)
    }

    /// 这一轮的一次请求看到了第 `seen` 条为止：排着的话序号不超过它的，这次请求带上了。
    pub(super) fn heard(&mut self, seen: u64) {
        let Some(turn) = self.turn.filter(|_| self.running.is_some()) else {
            return;
        };
        let picked: Vec<usize> = (0..self.entries.len())
            .filter(|&j| {
                let e = &self.entries[j];
                e.queued && e.seq.is_some_and(|s| s <= seen)
            })
            .collect();
        if picked.is_empty() {
            return;
        }
        // 前面那段时间线收起，这几句接在后面，接着的步另起一段。
        self.close_open_blocks();
        self.finish_segment();
        self.bring_in(&picked, turn);
    }

    /// 把 `picked` 这几条归到第 `turn` 轮：排着的挪到正文末尾，按先后拼成你说的一段话，一条一行；本来不是排着的
    /// 留在原位（`tui.md`「运行状态行和排队的消息」第 5 条）。
    pub(super) fn bring_in(&mut self, picked: &[usize], turn: u64) {
        let mut moved: Vec<Entry> = Vec::new();
        for &j in picked.iter().rev() {
            if self.entries[j].queued {
                moved.push(self.entries.remove(j));
                self.removed(j);
            } else {
                self.entries[j].turn = Some(turn);
            }
        }
        // 一句一句记着：Ctrl+C 打断、她还没开口的，照这个放回输入框（`takeback`）。
        self.opened_by = moved
            .iter()
            .rev()
            .map(|e| (e.text.clone(), e.pasted.clone()))
            .collect();
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

    /// 这一轮是排着的话开的、她还没开口（没说字、没做步，只在想也算没开口）：交回开它的那几句（字和粘贴块）。
    /// Ctrl+C 打断以后撤掉这一轮，那几句放回输入框（蓝图 `tui.md`「按键」`Ctrl+C`，2026-09-30 项目主人定只退回排着的）。
    pub fn takeback(&self) -> Option<Vec<(String, Vec<Chip>)>> {
        if self.opened_by.is_empty() || self.started_work() {
            return None;
        }
        Some(self.opened_by.clone())
    }

    /// 她还没开始做事的这一轮：交回开它的那几句，直接发的那一句也算（两下 `Esc` 打断以后撤掉、放回输入框，蓝图「按键」
    /// `Esc`，2026-10-07 项目主人定）。
    pub fn untouched(&self) -> Option<Vec<(String, Vec<Chip>)>> {
        let turn = self.turn?;
        if self.started_work() {
            return None;
        }
        if !self.opened_by.is_empty() {
            return Some(self.opened_by.clone());
        }
        let opening: Vec<(String, Vec<Chip>)> = self
            .entries
            .iter()
            .filter(|e| e.kind == Kind::User && e.turn == Some(turn))
            .map(|e| (e.text.clone(), e.pasted.clone()))
            .collect();
        (!opening.is_empty()).then_some(opening)
    }

    /// 这一轮在回答，她已经开始做事了：说了字或者调了工具（只在想不算）；没在回答的也算做了（不撤）。
    fn started_work(&self) -> bool {
        let (Some(_), Some(turn)) = (self.running, self.turn) else {
            return true;
        };
        self.entries
            .iter()
            .filter(|e| e.turn == Some(turn))
            .any(|e| {
                (e.kind == Kind::Reply && !e.text.trim().is_empty())
                    || e.segment.as_ref().is_some_and(|s| {
                        s.steps
                            .iter()
                            .any(|st| matches!(st.kind, StepKind::Tool { .. }))
                    })
            })
    }

    /// 第 `at` 条从正文里拿走了：记着的下标（在接的块、还没配上编号的调用、调用编号到那一步、正在压缩那一行）
    /// 在它后面的往前挪一格。
    pub(super) fn removed(&mut self, at: usize) {
        let shift = |i: &mut usize| {
            if *i > at {
                *i -= 1;
            }
        };
        for slot in self.blocks.values_mut() {
            match slot {
                Slot::Reply(i) | Slot::Step(i, _) => shift(i),
            }
        }
        for (i, _) in self.unnamed.iter_mut().chain(self.calls.values_mut()) {
            shift(i);
        }
        if let Some(i) = self.compacting.as_mut() {
            shift(i);
        }
    }
}
