//! 金额的测试（施工 8-15，`models.md`「守着它的」）：算式、分档、缺项不算、思考价不同不算、倍率照资料、用量是 0 的项不要价、
//! 币种照价格、本机的是 0；出处照资料的来源写；几种币种排先后。

use miyu_config::Layer;
use miyu_kernel::event::{Prices, Real, Usage};

use super::*;
use crate::catalog::{Price, Rates, Tier};
use crate::facts::{Fact, Facts, Source};

fn usage(uncached: u64, cache_read: u64, cache_write: u64, output: u64) -> Usage {
    Usage {
        uncached,
        cache_read,
        cache_write,
        output,
    }
}

fn rates(input: Option<f64>, output: Option<f64>, read: Option<f64>, write: Option<f64>) -> Rates {
    Rates {
        input,
        output,
        cache_read: read,
        cache_write: write,
    }
}

/// 图纸「样子」里那一份：输入 0.15、输出 0.6、读缓存 0.003，美元。
fn flash() -> Price {
    Price::of(rates(Some(0.15), Some(0.6), Some(0.003), None), "USD")
}

fn tariff(price: Price) -> Tariff {
    Tariff {
        price,
        multiplier: 1.0,
        source: "catalog:deepseek/deepseek-flash".to_string(),
    }
}

/// 驱动的保守默认：没有。
fn none<T>() -> Fact<Option<T>> {
    Fact {
        value: None,
        source: Source::Default,
    }
}

/// 只有价格、倍率要紧的一份资料。
fn facts(price: Option<Price>, source: Source, multiplier: f64) -> Facts {
    Facts {
        window: none(),
        max_output: none(),
        inputs: Fact {
            value: vec!["text".to_string()],
            source: Source::Default,
        },
        tools: none(),
        reasoning: none(),
        effort: none(),
        price: Fact {
            value: price,
            source,
        },
        multiplier: Fact {
            value: multiplier,
            source: Source::Default,
        },
        name: Fact {
            value: "m".to_string(),
            source: Source::Default,
        },
        status: none(),
    }
}

fn file(layer: Layer) -> String {
    match layer {
        Layer::System => "system/config.toml".to_string(),
        _ => "home/admin/settings.toml".to_string(),
    }
}

#[test]
fn the_drawing_adds_the_four_items_in_order() {
    let cost = tariff(flash()).cost(&usage(1843, 0, 0, 26)).unwrap();
    assert_eq!(
        cost.amount,
        Real::new((1843.0 * 0.15 + 26.0 * 0.6) / 1e6 * 1.0)
    );
    assert_eq!(
        serde_json::to_string(&cost).unwrap(),
        r#"{"amount":0.00029205,"currency":"USD","price":{"input":0.15,"output":0.6,"cache_read":0.003},"multiplier":1,"source":"catalog:deepseek/deepseek-flash"}"#
    );
    let mixed = tariff(flash()).cost(&usage(38, 1792, 0, 14)).unwrap();
    let sum = 38.0 * 0.15 + 1792.0 * 0.003 + 14.0 * 0.6;
    assert_eq!(mixed.amount, Real::new(sum / 1e6));
}

#[test]
fn the_highest_tier_strictly_exceeded_wins_and_fills_from_the_base() {
    let mut price = flash();
    price.tiers = vec![Tier {
        size: 128_000,
        rates: rates(Some(0.3), Some(1.2), None, None),
    }];
    price.over_200k = Some(rates(Some(0.6), None, None, None));
    let at = |input: u64| tariff(price.clone()).cost(&usage(input, 0, 0, 10)).unwrap();
    let base = at(128_000);
    assert_eq!(
        (base.above, base.price.input),
        (None, Some(Real::new(0.15)))
    );
    let tiered = at(128_001);
    assert_eq!(tiered.above, Some(128_000));
    assert_eq!(tiered.price.input, Some(Real::new(0.3)));
    assert_eq!(tiered.price.output, Some(Real::new(1.2)));
    assert_eq!(
        tiered.price.cache_read,
        Some(Real::new(0.003)),
        "档里没写的用底价"
    );
    let top = at(200_001);
    assert_eq!(top.above, Some(200_000));
    assert_eq!(top.price.input, Some(Real::new(0.6)));
    assert_eq!(
        top.price.output,
        Some(Real::new(0.6)),
        "没写的用底价的，不是下一档的"
    );
    assert_eq!(top.amount, Real::new((200_001.0 * 0.6 + 10.0 * 0.6) / 1e6));
    // 输入算三项：没命中的不多，命中的多，照样过线。
    let cached = tariff(price.clone())
        .cost(&usage(1, 128_000, 0, 0))
        .unwrap();
    assert_eq!(cached.above, Some(128_000));
}

#[test]
fn a_tier_written_at_the_same_size_comes_before_over_200k() {
    let mut price = flash();
    price.tiers = vec![Tier {
        size: 200_000,
        rates: rates(Some(0.5), None, None, None),
    }];
    price.over_200k = Some(rates(Some(0.9), None, None, None));
    let cost = tariff(price).cost(&usage(300_000, 0, 0, 0)).unwrap();
    assert_eq!(
        (cost.above, cost.price.input),
        (Some(200_000), Some(Real::new(0.5)))
    );
}

#[test]
fn a_used_item_without_a_price_means_no_cost() {
    // 写缓存没有价：用了的不算，没用的照算。
    assert_eq!(tariff(flash()).cost(&usage(10, 0, 5, 1)), None);
    assert!(tariff(flash()).cost(&usage(10, 0, 0, 1)).is_some());
    let no_output = Price::of(rates(Some(1.0), None, None, None), "USD");
    assert_eq!(tariff(no_output.clone()).cost(&usage(10, 0, 0, 1)), None);
    let free = tariff(no_output).cost(&usage(10, 0, 0, 0)).unwrap();
    assert_eq!(free.price.output, None, "只写有的几项");
    assert_eq!(free.amount, Real::new(10.0 / 1e6));
}

#[test]
fn a_zero_usage_costs_zero_without_asking_for_any_price() {
    let empty = Price::of(Rates::default(), "USD");
    let cost = tariff(empty).cost(&usage(0, 0, 0, 0)).unwrap();
    assert_eq!(cost.amount, Real::new(0.0));
    assert_eq!(cost.price, Prices::default());
}

#[test]
fn a_reasoning_price_unlike_the_output_price_means_no_cost() {
    let mut price = flash();
    price.reasoning = Some(2.0);
    assert_eq!(tariff(price.clone()).cost(&usage(10, 0, 0, 1)), None);
    price.reasoning = Some(0.6);
    assert!(tariff(price).cost(&usage(10, 0, 0, 1)).is_some());
}

#[test]
fn the_currency_follows_the_price_and_the_multiplier_applies() {
    let price = Price::of(rates(Some(2.0), Some(8.0), None, None), "CNY");
    let tariff = Tariff::of(
        &facts(
            Some(price),
            Source::Config {
                layer: Layer::System,
                line: 12,
            },
            0.5,
        ),
        &file,
    )
    .unwrap();
    assert_eq!(tariff.source, "config:system/config.toml:12");
    let cost = tariff.cost(&usage(1000, 0, 0, 100)).unwrap();
    assert_eq!(cost.currency, "CNY");
    assert_eq!(cost.multiplier, Real::new(0.5));
    assert_eq!(
        cost.amount,
        Real::new((1000.0 * 2.0 + 100.0 * 8.0) / 1e6 * 0.5)
    );
    let personal = Source::Config {
        layer: Layer::Personal,
        line: 3,
    };
    let price = Some(Price::of(Rates::default(), "USD"));
    assert_eq!(
        Tariff::of(&facts(price, personal, 1.0), &file)
            .unwrap()
            .source,
        "config:home/admin/settings.toml:3"
    );
}

#[test]
fn the_source_names_the_catalog_entry() {
    let source = Source::Catalog {
        entry: "deepseek/deepseek-flash".to_string(),
        layer: 2,
        fetched: "2026-10-01".to_string(),
    };
    let tariff = Tariff::of(&facts(Some(flash()), source, 1.0), &file).unwrap();
    assert_eq!(tariff.source, "catalog:deepseek/deepseek-flash");
}

#[test]
fn no_price_no_tariff() {
    assert_eq!(Tariff::of(&facts(None, Source::Default, 1.0), &file), None);
}

#[test]
fn a_local_service_costs_nothing() {
    let tariff = Tariff::of(&facts(Some(Price::free()), Source::Local, 3.0), &file).unwrap();
    let cost = tariff.cost(&usage(5000, 100, 7, 900)).unwrap();
    assert_eq!(
        serde_json::to_string(&cost).unwrap(),
        r#"{"amount":0,"currency":"USD","price":{"input":0,"output":0,"cache_read":0,"cache_write":0},"multiplier":3,"source":"local"}"#
    );
}

#[test]
fn the_preferred_currency_comes_first_then_by_code() {
    let mut amounts = vec![
        ("USD".to_string(), 1.0),
        ("CNY".to_string(), 2.0),
        ("EUR".to_string(), 3.0),
    ];
    ordered(&mut amounts, "EUR");
    let codes: Vec<&str> = amounts.iter().map(|(code, _)| code.as_str()).collect();
    assert_eq!(codes, ["EUR", "CNY", "USD"]);
    ordered(&mut amounts, "JPY");
    let codes: Vec<&str> = amounts.iter().map(|(code, _)| code.as_str()).collect();
    assert_eq!(codes, ["CNY", "EUR", "USD"]);
}
