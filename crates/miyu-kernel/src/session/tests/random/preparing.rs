//! 随机的提前压好（施工 6-11 上，`watch/prepare.rs`）：三个种子里有一个开着，另用一串随机数，原来那串输入不跟着错开；别的
//! 种子回合开始交关着，原来走得到的路照样走得到。
//!
//! 在路上的，多半送它的回报：还没报发出去的先报（偶尔先送一段增量，该算对不上），再一段段正文，说了话的才说完了；偶尔出错、
//! 偶尔送一条对不上的（该不理）。开着的种子里，五个回合里有一个交关着。

use super::*;
use crate::event::Purpose;

/// 这一次该送什么（`None` 是不送，别的输入照常来）。
pub(super) fn some_prepare(rng: &mut Rng, watch: &Watch) -> Option<Input> {
    let flight = watch.prepares.flight.as_ref()?;
    let upto = flight.upto;
    match rng.below(10) {
        0 => Some(ended(Seq::FIRST, None)),
        1 | 2 => None,
        3 if !flight.sent => Some(delta(upto, flight.next_delta())),
        _ if !flight.sent => Some(Input::AsideSent {
            at: at(47),
            purpose: Purpose::Compaction,
            upto,
            model: Model {
                endpoint: ProviderId::parse("deepseek").unwrap(),
                model: ModelName::parse("deepseek-v4").unwrap(),
            },
            request: ContentHash::of(b"prepare"),
        }),
        4 => Some(ended(
            upto,
            Some(CallError {
                class: ErrorClass::Retryable,
                message: "503".to_string(),
                status: None,
            }),
        )),
        5..=7 => Some(delta(upto, flight.next_delta())),
        _ if !flight.said() => Some(delta(upto, flight.next_delta())),
        _ => Some(ended(upto, None)),
    }
}

/// 挂接点的结果带上这一轮开不开：开着的种子里第 5、10……个序号开的回合关着，别的开着；不开的种子一律关着。
pub(super) fn with_prepare(watch: &Watch, mut input: Input) -> Input {
    if let Input::TurnStartHooksDone { turn, prepare, .. } = &mut input {
        *prepare = watch.prepares.seeded && turn.started().get() % 5 != 0;
    }
    input
}

/// 提前压 `upto` 的一段增量。
fn delta(upto: Seq, delta: Delta) -> Input {
    Input::AsideDelta {
        at: at(48),
        purpose: Purpose::Compaction,
        upto,
        delta,
    }
}

/// 提前压 `upto` 说完了：出错的带上分类和原话。
fn ended(upto: Seq, error: Option<CallError>) -> Input {
    Input::AsideEnded {
        at: at(49),
        purpose: Purpose::Compaction,
        upto,
        usage: None,
        cost: None,
        error,
    }
}
