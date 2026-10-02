//! 金额的测试（施工 8-15）：图纸上的写法读写一字不差；整数值写成整数、别的照最短的写法；没有的项不写；照位比相等；
//! 写坏的读不进来。

use super::*;
use crate::test_support::{rejected, round_trip};

/// 图纸「样子」里那一条的 `cost`。
const DRAWN: &str = r#"{"amount":0.00029205,"currency":"USD","price":{"input":0.15,"output":0.6,"cache_read":0.003},"multiplier":1,"source":"catalog:deepseek/deepseek-flash"}"#;

#[test]
fn the_drawing_round_trips() {
    round_trip::<Cost>(DRAWN);
    let cost: Cost = serde_json::from_str(DRAWN).unwrap();
    assert_eq!(cost.amount.get(), 0.000_292_05);
    assert_eq!(cost.multiplier.get(), 1.0);
    assert_eq!(cost.price.cache_write, None);
    assert_eq!(cost.above, None);
}

#[test]
fn a_tiered_price_says_which_tier() {
    round_trip::<Cost>(
        r#"{"amount":1.5,"currency":"CNY","price":{"input":4,"output":16},"multiplier":0.5,"source":"config:system/config.toml:12","above":200000}"#,
    );
}

/// 整数值写成整数：`1` 不写 `1.0`，`0` 不写 `0.0`；别的照最短能读回原值的写法。读进来的整数照样是那个小数。
#[test]
fn whole_numbers_are_written_as_integers() {
    for (number, written) in [
        (1.0, "1"),
        (0.0, "0"),
        (250.0, "250"),
        (0.6, "0.6"),
        (0.000_292_05, "0.00029205"),
        (1e-12, "1e-12"),
    ] {
        assert_eq!(serde_json::to_string(&Real::new(number)).unwrap(), written);
        let read: Real = serde_json::from_str(written).unwrap();
        assert_eq!(read, Real::new(number), "{written}");
    }
    let read: Real = serde_json::from_str("2").unwrap();
    assert_eq!(read.get(), 2.0);
}

#[test]
fn reals_compare_bit_for_bit() {
    assert_eq!(Real::new(0.1 + 0.2), Real::new(0.1 + 0.2));
    assert_ne!(Real::new(0.1 + 0.2), Real::new(0.3));
    assert_ne!(Real::new(0.0), Real::new(-0.0));
}

#[test]
fn local_services_cost_nothing() {
    round_trip::<Cost>(
        r#"{"amount":0,"currency":"USD","price":{"input":0,"output":0,"cache_read":0,"cache_write":0},"multiplier":1,"source":"local"}"#,
    );
}

#[test]
fn broken_costs_say_what_is_wrong() {
    rejected::<Cost>(
        r#"{"amount":"0.1","currency":"USD","price":{},"multiplier":1,"source":"local"}"#,
        "expected a number",
    );
    rejected::<Cost>(
        r#"{"currency":"USD","price":{},"multiplier":1,"source":"local"}"#,
        "missing field `amount`",
    );
    rejected::<Cost>(
        r#"{"amount":1,"currency":"USD","price":{"input":"x"},"multiplier":1,"source":"local"}"#,
        "expected a number",
    );
}
