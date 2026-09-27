use serde_json::json;

use super::*;

#[test]
fn every_request_of_the_turn_is_added_up() {
    let mut sum = Sum::default();
    assert!(!sum.seen);
    sum.add(&json!({"uncached": 1843, "cache_read": 0, "cache_write": 0, "output": 26}));
    sum.add(&json!({"uncached": 38, "cache_read": 1792, "cache_write": 5, "output": 14}));
    sum.add(&json!(null));
    assert!(sum.seen);
    assert_eq!(sum.input(), 1843 + 38 + 1792 + 5);
    assert_eq!(sum.output, 40);
    assert_eq!(
        sum.json(),
        json!({"input": 3678, "cache_read": 1792, "cache_write": 5, "output": 40})
    );
}

#[test]
fn the_hit_rate_rounds_to_a_whole_percent() {
    let sum = |uncached, cache_read| Sum {
        uncached,
        cache_read,
        seen: true,
        ..Sum::default()
    };
    assert_eq!(sum(38, 1792).percent(), Some(98));
    assert_eq!(sum(1, 1).percent(), Some(50));
    assert_eq!(sum(2, 1).percent(), Some(33));
    assert_eq!(sum(1, 2).percent(), Some(67));
    assert_eq!(sum(0, 0).percent(), None);
}

#[test]
fn big_numbers_get_commas_every_three_digits() {
    assert_eq!(thousands(0), "0");
    assert_eq!(thousands(999), "999");
    assert_eq!(thousands(1000), "1,000");
    assert_eq!(thousands(1830), "1,830");
    assert_eq!(thousands(1234567), "1,234,567");
}
