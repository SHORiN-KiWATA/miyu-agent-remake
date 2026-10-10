//! 回复的一块块（终端蓝图 `tui.md`「时间线」第 1、9、10 条）：正文另起一条、收起前面那一段；思考、调工具记进在进行的
//! 那一段。正文、思考出了字才有条目（空块不进落了盘的回复，流式时也不画）；调工具开始就有，落了盘的回复里没有它的
//! （打断时丢掉的半截）拿掉。一块什么时候算完：它的 `end`，或者同一次请求里下一块开始了，先到的算。回复记下的
//! 每一块的起止（`model.called` 的 `blocks`）定下每一块的开始时刻和思考用了多久，流式的、翻页的最后一样。

use miyu_kernel::accumulate::Kind;
use miyu_kernel::block::Block as Content;
use miyu_kernel::event::{Event, MessageAssistant, ModelCalled, ModelDelta, Piece};
use miyu_kernel::time::Timestamp;

use super::{Block, Projector};
use crate::entry::{Body, Entry, EntryId, Group, Reply, Thought};

impl Projector {
    /// 一段增量。
    pub(super) fn delta(&mut self, at: Timestamp, delta: &ModelDelta) {
        if self.request != Some(delta.seen) {
            self.close_blocks();
            self.blocks.clear();
            self.request = Some(delta.seen);
            self.heard(delta.seen);
        }
        let id = EntryId::block(delta.seen, delta.index);
        match &delta.piece {
            Piece::Start(kind) => {
                // 下一块开始了：前面的块都算收全了。
                self.close_blocks();
                let block = match kind {
                    Kind::Text => Block::Text(None),
                    Kind::Reasoning => Block::Thought(None),
                    Kind::ToolCall { name } => {
                        self.tool_step(id.clone(), name, None, String::new(), at);
                        Block::Tool(id)
                    }
                };
                self.blocks.insert(delta.index, block);
            }
            Piece::Text(text) => self.write(delta.index, id, text, at),
            Piece::End => self.close_block(delta.index),
        }
    }

    /// 一块的一段字：正文、思考头一回出字时才开条目。
    fn write(&mut self, index: usize, id: EntryId, text: &str, at: Timestamp) {
        match self.blocks.get(&index).cloned() {
            Some(Block::Text(None)) if !text.is_empty() => {
                self.reply_entry(id.clone(), text.to_string(), true, at);
                self.blocks.insert(index, Block::Text(Some(id)));
            }
            Some(Block::Thought(None)) if !text.is_empty() => {
                self.thought_step(id.clone(), text.to_string(), true, at);
                self.blocks.insert(index, Block::Thought(Some(id)));
            }
            Some(Block::Text(Some(id))) => self.append(&id, text, |body| match body {
                Body::Reply(reply) => Some(&mut reply.text),
                _ => None,
            }),
            Some(Block::Thought(Some(id))) => self.append(&id, text, |body| match body {
                Body::Thought(thought) => Some(&mut thought.text),
                _ => None,
            }),
            Some(Block::Tool(id)) => self.append(&id, text, |body| match body {
                Body::Tool(tool) => Some(&mut tool.args),
                _ => None,
            }),
            _ => {}
        }
    }

    /// 这次请求在写的块都算收全了。
    pub(super) fn close_blocks(&mut self) {
        let open: Vec<usize> = self.blocks.keys().copied().collect();
        for index in open {
            self.close_block(index);
        }
    }

    /// 一块收全了：正文写完，思考停表，调工具的参数读出来、等结果。收过的不再动。
    fn close_block(&mut self, index: usize) {
        let now = self.now;
        match self.blocks.get(&index).cloned() {
            Some(Block::Text(Some(id))) => self.touch_if(&id, |entry| match &mut entry.body {
                Body::Reply(reply) if reply.open => {
                    reply.open = false;
                    true
                }
                _ => false,
            }),
            Some(Block::Thought(Some(id))) => {
                let mut stopped = false;
                self.touch_if(&id, |entry| match &mut entry.body {
                    Body::Thought(thought) if thought.open => {
                        thought.open = false;
                        let took = now.unix_millis() - entry.at.unix_millis();
                        thought.took_ms = Some(u64::try_from(took).unwrap_or(0));
                        stopped = true;
                        true
                    }
                    _ => false,
                });
                if stopped {
                    self.ended.insert(id, now);
                }
            }
            Some(Block::Tool(id)) => self.ready(&id),
            _ => {}
        }
    }

    /// 一条回复落了盘：照它把流式的块定下来；流式时没见过的（翻页、没推过增量）照它开条目。
    pub(super) fn assistant(&mut self, event: &Event, reply: &MessageAssistant) {
        let seen = reply.seen;
        self.heard(seen);
        if self.request != Some(seen) {
            self.close_blocks();
            self.blocks.clear();
        }
        let streamed = std::mem::take(&mut self.blocks);
        self.request = None;
        let mut kept = Vec::new();
        let mut ids = Vec::new();
        for (position, block) in reply.blocks.iter().enumerate() {
            let index = reply.index_of(position);
            kept.push(index);
            let id = EntryId::block(seen, index);
            ids.push(self.settle_block(id, block, event.at));
        }
        // 流式时开了、回复里没有的：拿掉。
        for (index, block) in streamed {
            if kept.contains(&index) {
                continue;
            }
            if let Block::Tool(id) | Block::Text(Some(id)) | Block::Thought(Some(id)) = block {
                self.drop_step(&id);
            }
        }
        self.reply = Some((seen, ids));
        self.refresh_open_group();
    }

    /// 这次请求说完了、没写成回复（只开了工具调用就出错、一块都没留下）：流式时开的块都拿掉，翻页本来就没有。
    pub(super) fn discard_stream(&mut self, seen: miyu_kernel::id::Seq) {
        if self.request != Some(seen) {
            return;
        }
        self.request = None;
        for (_, block) in std::mem::take(&mut self.blocks) {
            if let Block::Tool(id) | Block::Text(Some(id)) | Block::Thought(Some(id)) = block {
                self.drop_step(&id);
            }
        }
    }

    /// 回复里的一块定下来，交回它的条目（没有条目的空思考是 `None`）。
    fn settle_block(&mut self, id: EntryId, block: &Content, at: Timestamp) -> Option<EntryId> {
        let exists = self.at.contains_key(&id);
        match block {
            Content::Text(text) => {
                if exists {
                    let full = text.text.clone();
                    self.touch_if(&id, |entry| match &mut entry.body {
                        Body::Reply(reply) if reply.open || reply.text != full => {
                            reply.text = full;
                            reply.open = false;
                            true
                        }
                        _ => false,
                    });
                } else {
                    self.reply_entry(id.clone(), text.text.clone(), false, at);
                }
                Some(id)
            }
            Content::Reasoning(reasoning) if reasoning.text.is_empty() => None,
            Content::Reasoning(reasoning) => {
                if exists {
                    let full = reasoning.text.clone();
                    self.touch_if(&id, |entry| match &mut entry.body {
                        Body::Thought(thought) if thought.open || thought.text != full => {
                            thought.text = full;
                            thought.open = false;
                            true
                        }
                        _ => false,
                    });
                } else {
                    self.thought_step(id.clone(), reasoning.text.clone(), false, at);
                }
                Some(id)
            }
            Content::ToolCall(call) => {
                if !exists {
                    self.tool_step(
                        id.clone(),
                        &call.name,
                        Some(call.call_id),
                        call.args.clone(),
                        at,
                    );
                }
                self.named(&id, call.call_id, &call.args);
                Some(id)
            }
            _ => None,
        }
    }

    /// 回复记下的每一块的起止：块的开始时刻是请求发出去的时刻加 `start_ms`，思考用了 `end_ms` 减 `start_ms`。
    pub(super) fn spans(&mut self, event: &Event, called: &ModelCalled) {
        let (Some((seen, ids)), Some(spans)) = (self.reply.take(), called.blocks.as_ref()) else {
            return;
        };
        if seen != called.seen {
            return;
        }
        let sent =
            event.at.unix_millis() - i64::try_from(called.duration_ms.unwrap_or(0)).unwrap_or(0);
        for (id, span) in ids.iter().zip(spans) {
            let Some(id) = id else {
                continue;
            };
            let start = i64::try_from(span.start_ms).unwrap_or(0);
            let Some(at) = Timestamp::from_unix_millis(sent + start) else {
                continue;
            };
            let took = span.end_ms.saturating_sub(span.start_ms);
            let mut thought = false;
            self.touch_if(id, |entry| {
                let mut changed = entry.at != at;
                entry.at = at;
                if let Body::Thought(t) = &mut entry.body {
                    thought = true;
                    changed |= t.took_ms != Some(took);
                    t.took_ms = Some(took);
                }
                changed
            });
            if thought
                && let Some(end) =
                    Timestamp::from_unix_millis(sent + i64::try_from(span.end_ms).unwrap_or(0))
            {
                self.ended.insert(id.clone(), end);
            }
        }
        self.refresh_groups_of(ids.iter().flatten().cloned().collect());
    }

    /// 正文另起一条：前面在进行的那一段先收起。
    fn reply_entry(&mut self, id: EntryId, text: String, open: bool, at: Timestamp) {
        self.close_group();
        if let Some(running) = self.turn.as_mut() {
            running.spoke = true;
        }
        let turn = self.turn.as_ref().map(|t| t.id);
        self.add(Entry {
            id,
            body: Body::Reply(Reply { text, open }),
            turn,
            hidden: false,
            at,
        });
    }

    /// 思考记进在进行的那一段。
    fn thought_step(&mut self, id: EntryId, text: String, open: bool, at: Timestamp) {
        let group = self.open_group(&id, at);
        let turn = self.turn.as_ref().map(|t| t.id);
        self.add(Entry {
            id: id.clone(),
            body: Body::Thought(Thought {
                group: group.clone(),
                text,
                open,
                took_ms: None,
            }),
            turn,
            hidden: false,
            at,
        });
        self.join_group(&group, id);
    }

    /// 在进行的那一段：没有的另起一段，编号照它的第一步。
    pub(super) fn open_group(&mut self, first: &EntryId, at: Timestamp) -> EntryId {
        if let Some(group) = &self.group {
            return group.clone();
        }
        let id = EntryId::group(first);
        let turn = self.turn.as_ref().map(|t| t.id);
        self.add(Entry {
            id: id.clone(),
            body: Body::Group(Group {
                open: true,
                ..Group::default()
            }),
            turn,
            hidden: false,
            at,
        });
        self.group = Some(id.clone());
        id
    }

    /// 一步记进一段。
    pub(super) fn join_group(&mut self, group: &EntryId, step: EntryId) {
        self.touch(group, |entry| {
            if let Body::Group(g) = &mut entry.body {
                g.steps.push(step);
            }
        });
        self.refresh_group(group, true);
    }

    /// 拿掉一步：它所在的那一段跟着少一步，空了的那一段也拿掉。
    fn drop_step(&mut self, id: &EntryId) {
        let group = match self.get(id).map(|e| &e.body) {
            Some(Body::Thought(t)) => Some(t.group.clone()),
            Some(Body::Tool(t)) => Some(t.group.clone()),
            _ => None,
        };
        self.remove(id);
        self.ended.remove(id);
        let Some(group) = group else {
            return;
        };
        let mut empty = false;
        self.touch(&group, |entry| {
            if let Body::Group(g) = &mut entry.body {
                g.steps.retain(|s| s != id);
                empty = g.steps.is_empty();
            }
        });
        if empty {
            if self.group.as_ref() == Some(&group) {
                self.group = None;
            }
            self.remove(&group);
        }
    }

    /// 在进行的那一段重算一遍。
    pub(super) fn refresh_open_group(&mut self) {
        if let Some(group) = self.group.clone() {
            self.refresh_group(&group, true);
        }
    }

    /// 这几步所在的段重算一遍。
    pub(super) fn refresh_groups_of(&mut self, steps: Vec<EntryId>) {
        let mut groups: Vec<EntryId> = steps
            .iter()
            .filter_map(|s| match self.get(s).map(|e| &e.body) {
                Some(Body::Thought(t)) => Some(t.group.clone()),
                Some(Body::Tool(t)) => Some(t.group.clone()),
                _ => None,
            })
            .collect();
        groups.dedup();
        for group in groups {
            let open = self.group.as_ref() == Some(&group);
            self.refresh_group(&group, open);
        }
    }
}
