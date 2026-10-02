//! 随机的回顾（施工 3-8 四补，`watch/recap.rs`）：另用一串随机数，原来那串输入不跟着错开。
//!
//! 有一次在路上的，多半送它的回报：还没报发出去的先报（偶尔先送一段增量，该算增量对不上），再一段段正文，说了话的才说完了；
//! 偶尔一段对不上的增量、出错；偶尔送一条对不上的回报（该不理），偶尔再要一次（该并进去）。没有在路上的，二十五回里一回
//! 要一句回顾，刚写好一句、还没别的事的，三回里一回再要一次（该交回这一句）。

use super::*;
use crate::event::Purpose;

/// 这一次该送什么（`None` 是不送，别的输入照常来）。
pub(super) fn some_recap(rng: &mut Rng, watch: &Watch, next_id: &mut u64) -> Option<Input> {
    let Some(flight) = watch.recaps.flight.as_ref() else {
        // 刚写好一句、还没别的事：多半再要一次，该交回这一句。
        let chance = if watch.just_recapped() { 3 } else { 25 };
        return (rng.below(chance) == 0).then(|| recap(next_id));
    };
    let upto = flight.upto;
    match rng.below(10) {
        0 => Some(ended(Seq::FIRST, None)),
        1 => Some(recap(next_id)),
        2 | 3 => None,
        4 if !flight.sent => Some(delta(upto, flight.next_delta())),
        _ if !flight.sent => Some(Input::AsideSent {
            at: at(47),
            purpose: Purpose::Recap,
            upto,
            model: model(),
            request: ContentHash::of(b"recap"),
        }),
        4 => Some(delta(upto, Delta::End { index: 9 })),
        5 => Some(ended(
            upto,
            Some(CallError {
                class: ErrorClass::Retryable,
                message: "503".to_string(),
                status: None,
            }),
        )),
        6 | 7 => Some(delta(upto, flight.next_delta())),
        _ if !flight.said() => Some(delta(upto, flight.next_delta())),
        _ => Some(ended(upto, None)),
    }
}

/// 跑完以前，在路上的那一次说完：报过发出去了的说完了，没报的先报。每个命令都要有回应。
pub(super) fn finish_recap(watch: &Watch) -> Vec<Input> {
    let Some(flight) = watch.recaps.flight.as_ref() else {
        return Vec::new();
    };
    let mut inputs = Vec::new();
    if !flight.sent {
        inputs.push(Input::AsideSent {
            at: at(47),
            purpose: Purpose::Recap,
            upto: flight.upto,
            model: model(),
            request: ContentHash::of(b"recap"),
        });
    }
    inputs.push(ended(flight.upto, None));
    inputs
}

/// 要一句回顾：新的编号。
fn recap(next_id: &mut u64) -> Input {
    let n = next_command(next_id);
    Input::Command(Received {
        id: id(n),
        by: alice(),
        at: at(n % 60),
        command: Command::Recap,
    })
}

/// 回顾 `upto` 的一段增量。
fn delta(upto: Seq, delta: Delta) -> Input {
    Input::AsideDelta {
        at: at(48),
        purpose: Purpose::Recap,
        upto,
        delta,
    }
}

/// 回顾 `upto` 说完了：出错的带上分类和原话。
fn ended(upto: Seq, error: Option<CallError>) -> Input {
    Input::AsideEnded {
        at: at(49),
        purpose: Purpose::Recap,
        upto,
        usage: None,
        cost: None,
        error,
    }
}

/// 替身的模型。
fn model() -> Model {
    Model {
        endpoint: ProviderId::parse("deepseek").unwrap(),
        model: ModelName::parse("deepseek-v4").unwrap(),
    }
}
