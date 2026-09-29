//! 随机测试的策略和模型的限额（施工 6-2 上）：压缩的数很小，限额另用一串随机数交，好让压缩和别的输入交错。压后重建
//! 的数也很小，几段的模板短，一眼认得出（施工 6-5）。

use super::*;

/// 随机测试的策略：一个回合最多请求 [`STEP_LIMIT`] 次；`attended` 是有没有人能确认、回答。压缩的数很小：替身的
/// 组装一条事件约五个 token，几十条就过线（施工 6-2 上）。熔断的数调松了（施工 6-6 上）：8 个回合内又到线算快、连着 2 次就暂停，
/// 随机的会话难得连着压好几次，照出厂的 3、3 长跑也走不到暂停。
/// `isolate` 是快照里有没有隔离式那句 system（施工 6-6 下）：没有的，摘要回复里调了工具照失败算，连续失败才走得到。
pub(super) fn random_policy(attended: bool, isolate: bool) -> Policy {
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
        rebuild: Some(crate::session::Rebuild {
            files: 2,
            file_tokens: 20,
            total: 30,
            min_window: 100,
            candidates: 3,
        }),
        pause: Some(crate::session::Pause {
            failures: 3,
            turns: 8,
            refills: 2,
        }),
        shorten: Some(crate::session::Shorten {
            tries: 3,
            percent: 20,
        }),
        isolate,
    });
    let template = |source: &str| crate::template::Template::parse(source).unwrap();
    limited.notes = Some(crate::session::Notes {
        files: template("<files/>"),
        files_more: template("<more {count}/>"),
        retrieve: template("<retrieve {upto}/>"),
        too_large: template("<too-large {files}/>"),
        uncovered: Some(template("<uncovered {from}-{to}/>")),
    });
    limited
}

/// 在路上的请求报超长，另用一串随机数：原来那串输入不跟着错开。只在捣乱的种子里，有请求在路上时，十回里有一回；
/// 一半说了超多少。摘要请求的走截短（施工 6-6 中），主请求的走被动压缩（施工 6-7）。
pub(super) fn some_overflow(rng: &mut Rng, watch: &Watch) -> Option<Input> {
    let seen = watch
        .summarizing()
        .or_else(|| watch.asking_main())
        .filter(|_| !watch.calm)?;
    if rng.below(10) != 0 {
        return None;
    }
    let excess = (rng.below(2) == 0).then(|| 5 * (1 + rng.below(20)));
    Some(Input::ModelEnded {
        at: at(45),
        seen,
        usage: None,
        error: Some(CallError {
            class: ErrorClass::ContextTooLong,
            message: "413".to_string(),
        }),
        wait_ms: None,
        excess,
    })
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
        images: None,
    }))
}
