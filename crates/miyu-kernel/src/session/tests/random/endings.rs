//! 模型的一次请求怎么收场（随机测试的执行器替身）：多半正常说完，四回里有一回出错，照几类抽。摘要请求报超长另用一串随机数
//! （`compacting.rs` 的 `some_overflow`）。

use super::*;

/// 说完了的结局：四回里有一回出错，出错的带着供应商说的要等多久（施工 3-5 下）。多半是可以重试的
/// 503；也有限速的（等 3 秒，或者 10 分钟：太久不等）、认证失败的（不重试）。
pub(super) fn some_ending(rng: &mut Rng) -> (Option<CallError>, Option<u64>) {
    if rng.below(4) > 0 {
        return (None, None);
    }
    let (class, message, wait) = match rng.below(8) {
        0 => (ErrorClass::Auth, "401", None),
        1 => (ErrorClass::RateLimited, "429", Some(3000)),
        2 => (ErrorClass::RateLimited, "429", Some(600_000)),
        _ => (ErrorClass::Retryable, "503", None),
    };
    let error = CallError {
        class,
        message: message.to_string(),
    };
    (Some(error), wait)
}
