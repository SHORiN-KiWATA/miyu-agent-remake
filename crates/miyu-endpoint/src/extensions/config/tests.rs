//! 变了的键（施工 9-4 下下）：新的、改了的照新值，没了的是 `null`，没变的不放。

use serde_json::json;

use super::*;

#[test]
fn only_changed_keys_are_listed_and_gone_ones_are_null() {
    let map = |pairs: &[(&str, Value)]| -> BTreeMap<String, Value> {
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_string(), value.clone()))
            .collect()
    };
    let before = map(&[
        ("a.port", json!(1)),
        ("a.token", json!("x")),
        ("a.same", json!(true)),
    ]);
    let after = map(&[
        ("a.port", json!(2)),
        ("a.same", json!(true)),
        ("a.new", json!("n")),
    ]);
    assert_eq!(
        Value::Object(changes(&before, &after)),
        json!({"a.port": 2, "a.token": null, "a.new": "n"})
    );
    assert!(changes(&after, &after).is_empty());
}
