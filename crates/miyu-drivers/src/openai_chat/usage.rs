//! 各家的用量写法归成内核的四项（`05-内核接口.md` 第七节「怎么解码」）：`prompt_tokens` 含命中的
//! 和写入的，没命中的是减出来的；输出含思考。

use miyu_kernel::event::Usage;
use serde_json::Value;

/// 一段用量。没有 `prompt_tokens` 的不算用量：有的网关每一段都带一个空的 `usage`。
pub(super) fn usage(value: &Value) -> Option<Usage> {
    let prompt = number(value, "/prompt_tokens")?;
    let cache_read = number(value, "/prompt_tokens_details/cached_tokens")
        .or_else(|| number(value, "/prompt_cache_hit_tokens"))
        .or_else(|| number(value, "/cached_tokens"))
        .unwrap_or(0);
    let cache_write = number(value, "/prompt_tokens_details/cache_write_tokens").unwrap_or(0);
    Some(Usage {
        uncached: prompt
            .saturating_sub(cache_read)
            .saturating_sub(cache_write),
        cache_read,
        cache_write,
        output: number(value, "/completion_tokens").unwrap_or(0),
    })
}

fn number(value: &Value, pointer: &str) -> Option<u64> {
    value.pointer(pointer).and_then(Value::as_u64)
}
