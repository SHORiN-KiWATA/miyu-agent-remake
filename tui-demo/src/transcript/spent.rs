//! 一次请求用了多少、花了多少：记进这个会话的累计和这一轮的（蓝图「用量」，核心 8-15 的金额）。

use super::Transcript;
use crate::core::Usage;

impl Transcript {
    /// 主请求、摘要请求的用量：加进累计和这一轮的，上下文照这一次的输入加输出。
    pub(super) fn used(&mut self, usage: Usage) {
        for sum in [&mut self.total, &mut self.turn_usage] {
            sum.uncached += usage.uncached;
            sum.cache_read += usage.cache_read;
            sum.cache_write += usage.cache_write;
            sum.output += usage.output;
        }
        self.context = usage.input() + usage.output;
    }
}
