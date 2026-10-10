//! 落了盘的和瞬时的照交出来的先后（施工 9-8 上）：视图投影照它一条条喂，模拟头接着推送看到的样子。

use super::Stage;
use crate::event::{Event, Transient};

impl Stage {
    /// 落了盘的和瞬时的照交出来的先后。崩了以后没落盘的不在里面。
    pub fn stream(&self) -> Vec<Streamed<'_>> {
        self.order
            .iter()
            .filter_map(|&(stored, i)| match stored {
                true => self.log.get(i).map(Streamed::Event),
                false => self.transients.get(i).map(Streamed::Transient),
            })
            .collect()
    }
}

/// 推给头的一条：落了盘的事件，或者瞬时的（[`Stage::stream`]）。
#[derive(Debug, Clone, Copy)]
pub enum Streamed<'a> {
    /// 落了盘的。
    Event(&'a Event),
    /// 瞬时的。
    Transient(&'a Transient),
}
