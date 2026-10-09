//! 用量归成内核的四项（`docs/blueprint/drivers/anthropic.md`「怎么走：解码」的用量那张表）：`message_start` 的
//! `message.usage` 先记，`message_delta` 的 `usage` 里有哪项盖哪项。`input_tokens` 报的就是没命中的那一截，不用减；输出含思考。

use miyu_kernel::event::Usage;
use serde_json::Value;

/// 记下的四项：每一项最后一次报的，只认非负整数。
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Reported {
    input: Option<u64>,
    cache_read: Option<u64>,
    cache_write: Option<u64>,
    output: Option<u64>,
}

impl Reported {
    /// 一段用量：有哪项盖哪项。
    pub(super) fn update(&mut self, value: &Value) {
        let number = |field: &str| value.get(field).and_then(Value::as_u64);
        let fields = [
            (&mut self.input, "input_tokens"),
            (&mut self.cache_read, "cache_read_input_tokens"),
            (&mut self.cache_write, "cache_creation_input_tokens"),
            (&mut self.output, "output_tokens"),
        ];
        for (slot, field) in fields {
            if let Some(found) = number(field) {
                *slot = Some(found);
            }
        }
    }

    /// 内核的四项；没有 `input_tokens` 的不算用量。
    pub(super) fn usage(self) -> Option<Usage> {
        Some(Usage {
            uncached: self.input?,
            cache_read: self.cache_read.unwrap_or(0),
            cache_write: self.cache_write.unwrap_or(0),
            output: self.output.unwrap_or(0),
            reasoning: None,
        })
    }
}
