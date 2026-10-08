//! 摘要写了多少字（施工 6-3 下的进度；6-11 下起提前压的那一次也数，等它的时候照它推进度）：只数正文块的字，思考、工具
//! 调用不数。

use std::collections::BTreeSet;

use crate::accumulate::{Delta, Kind};

/// 正文块的编号，和到这时收到的正文字数。
#[derive(Debug, Default)]
pub(in crate::session) struct Written {
    texts: BTreeSet<usize>,
    chars: u64,
}

impl Written {
    /// 收到一段增量：正文块的开始记下编号，正文块的字数上。数了字的交回真。
    pub(in crate::session) fn take(&mut self, delta: &Delta) -> bool {
        match delta {
            Delta::Start {
                index,
                kind: Kind::Text,
            } => {
                self.texts.insert(*index);
                false
            }
            Delta::Text { index, text } if self.texts.contains(index) => {
                let chars = u64::try_from(text.chars().count()).unwrap_or(u64::MAX);
                self.chars = self.chars.saturating_add(chars);
                true
            }
            _ => false,
        }
    }

    /// 到这时收到的正文字数。
    pub(in crate::session) fn chars(&self) -> u64 {
        self.chars
    }
}
