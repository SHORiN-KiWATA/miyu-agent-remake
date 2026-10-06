//! 熔断的单元测试（施工 6-6 上）：哪几种摘要请求的失败算数。整轮怎么走在 `tests/scenario/breaker.rs`。

use super::*;
use crate::id::Seq;

/// 一次摘要请求的记录：`compaction` 是哪一种压缩，`result` 是结果。
fn called(compaction: Option<CompactTrigger>, result: CallResult) -> ModelCalled {
    ModelCalled {
        seen: Seq::new(8).unwrap(),
        endpoint: None,
        model: None,
        request: None,
        messages: 3,
        first_difference: None,
        usage: None,
        cost: None,
        first_token_ms: None,
        duration_ms: None,
        blocks: None,
        result,
        error: None,
        compaction,
        purpose: None,
    }
}

#[test]
fn only_failed_automatic_summary_requests_count() {
    for (compaction, result, counts) in [
        (Some(CompactTrigger::Auto), CallResult::Error, true),
        (Some(CompactTrigger::Overflow), CallResult::Error, true),
        // 人要的压缩失败了，人就在跟前。
        (Some(CompactTrigger::Manual), CallResult::Error, false),
        (
            Some(CompactTrigger::Other("scheduled".to_string())),
            CallResult::Error,
            false,
        ),
        (Some(CompactTrigger::Auto), CallResult::Ok, false),
        (Some(CompactTrigger::Auto), CallResult::Interrupted, false),
        // 主请求出的错。
        (None, CallResult::Error, false),
    ] {
        assert_eq!(
            failed_automatically(&called(compaction.clone(), result.clone())),
            counts,
            "{compaction:?} {result:?}"
        );
    }
}
