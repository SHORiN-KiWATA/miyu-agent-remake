//! 金额（施工 8-15，`docs/blueprint/kernel/session.md`「收回复」）：执行器随「说完了」交来的金额原样记进 `model.called`，
//! 排在用量后面；出错的带着用量和金额的也照记（钱已经花了）；没交的不写；被打断的没有。

use super::executor::*;
use super::*;
use crate::event::{CallResult, Cost, ErrorClass, ModelCalled, Prices, Real};

/// 一份金额。
fn cost(amount: f64) -> Cost {
    Cost {
        amount: Real::new(amount),
        currency: "USD".to_string(),
        price: Prices {
            input: Some(Real::new(0.15)),
            output: Some(Real::new(0.6)),
            cache_read: None,
            cache_write: None,
        },
        multiplier: Real::new(1.0),
        source: "catalog:deepseek/deepseek-flash".to_string(),
        above: None,
    }
}

/// 请求 `seen` 说完了，带用量、金额 `cost`，出错的带 `error`。
fn priced(seen: u64, cost: Option<Cost>, error: Option<ErrorClass>) -> Input {
    Input::ModelEnded {
        at: at(45),
        seen: seq(seen),
        usage: Some(usage()),
        cost,
        error: error.map(|class| crate::event::CallError {
            class,
            message: "boom".to_string(),
            status: None,
        }),
        wait_ms: None,
        excess: None,
        failover: false,
    }
}

/// 这批里那一条 `model.called`。
fn recorded(actions: &[Action]) -> ModelCalled {
    appended_events(actions)
        .into_iter()
        .find_map(|event| match event.body {
            Body::ModelCalled(called) => Some(called),
            _ => None,
        })
        .expect("记了一条 model.called")
}

#[test]
fn the_cost_handed_in_is_recorded_as_is_after_the_usage() {
    let mut session = asking();
    session.handle(sent(5));
    for piece in words(0, "好") {
        session.handle(delta(5, 41, piece));
    }
    let done = session.handle(priced(5, Some(cost(0.000_292_05)), None));
    let called = recorded(&done);
    assert_eq!(called.cost.as_deref(), Some(&cost(0.000_292_05)));
    let line = appended_events(&done)
        .into_iter()
        .find(|event| matches!(event.body, Body::ModelCalled(_)))
        .unwrap()
        .to_line();
    let usage_at = line.find("\"usage\"").unwrap();
    let cost_at = line.find("\"cost\"").unwrap();
    let first_token_at = line.find("\"first_token_ms\"").unwrap();
    assert!(usage_at < cost_at && cost_at < first_token_at, "{line}");
}

#[test]
fn no_cost_handed_in_writes_none() {
    let mut session = asking();
    let done = answer(&mut session, 5, "好");
    assert_eq!(recorded(&done).cost, None);
    assert!(!appended_events(&done)[1].to_line().contains("cost"));
}

#[test]
fn a_failed_request_that_was_charged_keeps_its_cost() {
    let mut session = asking();
    session.handle(sent(5));
    let done = session.handle(priced(5, Some(cost(0.01)), Some(ErrorClass::ContentPolicy)));
    let called = recorded(&done);
    assert_eq!(called.result, CallResult::Error);
    assert_eq!(called.cost.as_deref(), Some(&cost(0.01)));
}

#[test]
fn an_interrupted_request_has_no_cost() {
    let mut session = asking();
    session.handle(sent(5));
    for piece in words(0, "好") {
        session.handle(delta(5, 41, piece));
    }
    let done = session.handle(interrupt(9));
    let called = recorded(&done);
    assert_eq!(called.result, CallResult::Interrupted);
    assert_eq!(called.cost, None);
}
