//! 模型的一次请求怎么收场（随机测试的执行器替身）：多半正常说完，四回里有一回出错，照几类抽。摘要请求报超长另用一串随机数
//! （`compacting.rs` 的 `some_overflow`）。

use super::*;

/// 说完了的结局：四回里有一回出错，出错的带着供应商说的要等多久（施工 3-5 下）、端口说换没换端点（施工 8-9）。多半是可以
/// 重试的 503；也有限速的（等 3 秒，或者 10 分钟：太久不等）、认证失败的（不重试）；端口换了端点的（认证失败当场换，限速的
/// 别的都在冷却、等 3 秒），全在冷却的（等最早恢复的 3 秒，或者 10 分钟：太久不等）。
pub(super) fn some_ending(rng: &mut Rng) -> (Option<CallError>, Option<u64>, bool) {
    if rng.below(4) > 0 {
        return (None, None, false);
    }
    let (class, message, wait, failover) = match rng.below(16) {
        0 => (ErrorClass::Auth, "401", None, false),
        1 => (ErrorClass::RateLimited, "429", Some(3000), false),
        2 => (ErrorClass::RateLimited, "429", Some(600_000), false),
        3 => (ErrorClass::Auth, "401", None, true),
        4 => (ErrorClass::RateLimited, "429", Some(3000), true),
        5 => (
            ErrorClass::Cooling,
            "all candidates cooling",
            Some(3000),
            false,
        ),
        6 => (
            ErrorClass::Cooling,
            "all candidates cooling",
            Some(600_000),
            false,
        ),
        _ => (ErrorClass::Retryable, "503", None, false),
    };
    let error = CallError {
        class,
        message: message.to_string(),
        status: None,
    };
    (Some(error), wait, failover)
}
