//! 一块块流式的字接到哪里去（`tui.md`「时间线」第 1、9、10 条）：回答另起一条；思考、调工具记进在进行的那一段；
//! 一块什么时候算收全。

use serde_json::Value;

use super::{Kind, Segment, Slot, Step, StepKind, ToolState, Transcript};
use crate::core::Block;

impl Transcript {
    /// 一块开始了：回答另起一条，收起前面那一段；思考、调工具记进在进行的那一段。
    pub(super) fn open_block(&mut self, block: Block) -> Slot {
        let kind = match block {
            Block::Text => {
                self.finish_segment();
                self.push(Kind::Reply, String::new());
                return Slot::Reply(self.entries.len() - 1);
            }
            Block::Reasoning => StepKind::Thought {
                text: String::new(),
            },
            Block::ToolCall(name) => StepKind::Tool {
                name,
                args: String::new(),
                parsed: Value::Null,
                state: ToolState::Preparing,
                output: String::new(),
                said: None,
            },
        };
        let is_tool = matches!(kind, StepKind::Tool { .. });
        let now = self.clock.now();
        let entry = self.open_segment();
        let steps = &mut self.entries[entry]
            .segment
            .get_or_insert_with(Segment::new)
            .steps;
        steps.push(Step::starting(kind, now));
        let at = (entry, steps.len() - 1);
        if is_tool {
            self.unnamed.push(at);
        }
        Slot::Step(at.0, at.1)
    }

    pub(super) fn delta(&mut self, index: u64, piece: &str) {
        match self.blocks.get(&index).copied() {
            Some(Slot::Reply(i)) => {
                if let Some(entry) = self.entries.get_mut(i) {
                    entry.text.push_str(piece);
                }
            }
            Some(Slot::Step(i, j)) => match self.step_mut((i, j)).map(|s| &mut s.kind) {
                Some(StepKind::Thought { text }) => text.push_str(piece),
                Some(StepKind::Tool { args, .. }) => args.push_str(piece),
                None => {}
            },
            None => {}
        }
    }

    /// 收到这一块的 `end`。
    pub(super) fn block_end(&mut self, index: u64) {
        if let Some(Slot::Step(i, j)) = self.blocks.get(&index).copied() {
            self.close((i, j));
        }
    }

    /// 这次请求里前面还开着的块都算收全了：回复是一块接一块出来的，OpenAI 兼容接口的驱动要等整个回复收完
    /// 才一起报 `end`（蓝图 `tui.md`「时间线」第 10 条）。
    pub(super) fn close_open_blocks(&mut self) {
        let open: Vec<_> = self
            .blocks
            .values()
            .filter_map(|slot| match slot {
                Slot::Step(i, j) => Some((*i, *j)),
                Slot::Reply(_) => None,
            })
            .collect();
        for at in open {
            self.close(at);
        }
    }

    /// 一块收全了：思考停表；调工具的参数读出来，等结果。收过的再收一次不变，有了结果的不退回去。
    pub(super) fn close(&mut self, at: (usize, usize)) {
        let now = self.clock.now();
        let Some(step) = self.step_mut(at) else {
            return;
        };
        match &mut step.kind {
            StepKind::Thought { .. } => step.stop_at(now),
            StepKind::Tool {
                args,
                parsed,
                state,
                ..
            } if *state == ToolState::Preparing => {
                *parsed = serde_json::from_str(args).unwrap_or(Value::Null);
                *state = ToolState::Running;
            }
            StepKind::Tool { .. } => {}
        }
    }

    /// 在进行的那一段：最后一条是这一轮没结束的一段就是它，不是就另起一段。交回它在正文里是第几条。
    /// 排着队的话不算（它们还没进正文，`queue.rs`）。
    pub(super) fn open_segment(&mut self) -> usize {
        let open = self.tail().filter(|&i| {
            let e = &self.entries[i];
            e.turn == self.turn && e.segment.as_ref().is_some_and(|s| !s.finished)
        });
        if let Some(i) = open {
            return i;
        }
        self.push(Kind::Steps, String::new());
        if let Some(last) = self.entries.last_mut() {
            last.segment = Some(Segment::new());
        }
        self.entries.len() - 1
    }

    /// 收起在进行的那一段（说话就收起）。
    pub(super) fn finish_segment(&mut self) {
        let now = self.clock.now();
        let last = self.tail().and_then(|i| self.entries.get_mut(i));
        if let Some(segment) = last.and_then(|e| e.segment.as_mut()) {
            segment.finish_at(now);
        }
    }

    pub(super) fn step_mut(&mut self, (i, j): (usize, usize)) -> Option<&mut Step> {
        self.entries.get_mut(i)?.segment.as_mut()?.steps.get_mut(j)
    }
}
