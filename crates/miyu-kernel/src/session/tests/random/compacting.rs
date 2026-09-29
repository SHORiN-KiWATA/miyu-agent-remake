//! 随机测试的策略和模型的限额（施工 6-2 上）：压缩的数很小，限额另用一串随机数交，好让压缩和别的输入交错。

use super::*;

/// 随机测试的策略：一个回合最多请求 [`STEP_LIMIT`] 次；`attended` 是有没有人能确认、回答。压缩的数很小：替身的
/// 组装一条事件约五个 token，几十条就过线（施工 6-2 上）。
pub(super) fn random_policy(attended: bool) -> Policy {
    let mut limited = policy();
    limited.step_limit = Some(STEP_LIMIT);
    limited.attended = attended;
    limited.compaction = Some(Compaction {
        reserve_cap: 10,
        margin: 10,
        tail: 30,
        price: crate::estimate::Flat {
            image: 50,
            file: 50,
        },
    });
    limited
}

/// 模型的限额，另用一串随机数：原来那串输入不跟着错开。三十回里有一回；窗口多半小到几十条事件就过线，偶尔没有
/// 窗口、窗口很大（施工 6-2 上）。
pub(super) fn some_limits(rng: &mut Rng) -> Option<Input> {
    if rng.below(30) != 0 {
        return None;
    }
    let window = match rng.below(6) {
        0 => None,
        1 => Some(100_000),
        2 => Some(120),
        3 => Some(200),
        _ => Some(300),
    };
    let max_output = (rng.below(2) == 0).then_some(5);
    Some(Input::Limits(Limits {
        model: Model {
            endpoint: ProviderId::parse("deepseek").unwrap(),
            model: ModelName::parse("deepseek-v4").unwrap(),
        },
        window,
        max_output,
    }))
}
