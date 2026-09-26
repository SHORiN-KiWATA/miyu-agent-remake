//! 最近接受的命令编号（`docs/designs/02-内核.md` 第四节「输入、动作、命令怎么写」，不变量 9）。
//!
//! 同一个编号再来，照上一次回应，不再产生事件。只记最近的 [`CAPACITY`] 个，不随日志变长：
//! 断线重发发生在几秒、几分钟之内，再早的编号再来，当新命令。

use std::collections::{BTreeMap, VecDeque};

use crate::id::{CommandId, Seq};

/// 最多记几个编号。
pub(crate) const CAPACITY: usize = 1024;

/// 最近接受的命令编号，和它们产生的事件的序号。
#[derive(Debug, Clone, Default)]
pub(crate) struct Recent {
    /// 编号到它产生的事件的序号。
    events: BTreeMap<CommandId, Vec<Seq>>,
    /// 编号接受的先后，最早的在前，满了就从这头丢。
    order: VecDeque<CommandId>,
}

impl Recent {
    /// 这个编号最近接受过的话，它产生的事件的序号。
    pub(crate) fn get(&self, id: &CommandId) -> Option<&[Seq]> {
        self.events.get(id).map(Vec::as_slice)
    }

    /// 记下一个接受了的命令。满了，丢掉最早的那个。
    pub(crate) fn insert(&mut self, id: CommandId, events: Vec<Seq>) {
        if self.events.insert(id.clone(), events).is_none() {
            self.order.push_back(id);
        }
        while self.order.len() > CAPACITY {
            if let Some(oldest) = self.order.pop_front() {
                self.events.remove(&oldest);
            }
        }
    }
}
