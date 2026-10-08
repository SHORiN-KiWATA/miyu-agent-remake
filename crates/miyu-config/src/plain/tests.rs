//! 没有清单的 TOML 读一项、协议上的值换成要写的值（施工 P-3 中）。

use serde_json::json;

use super::{from_json, json_at};
use crate::value::Value;

const TEXT: &str = "[preset]\nname = { zh = \"开发\" }\nunlisted = \"off\"\n\n[software]\nnet = true\nmemory.extra = 1\n\n[tools]\nshell = false\n";

#[test]
fn values_are_read_through_tables_inline_tables_and_dotted_keys() {
    assert_eq!(json_at(TEXT, "preset.unlisted"), Some(json!("off")));
    assert_eq!(
        json_at(TEXT, "preset.name.zh"),
        Some(json!("开发")),
        "行内表"
    );
    assert_eq!(json_at(TEXT, "software.net"), Some(json!(true)));
    assert_eq!(
        json_at(TEXT, "software.memory.extra"),
        Some(json!(1)),
        "点号连着的键"
    );
    assert_eq!(json_at(TEXT, "tools.shell"), Some(json!(false)));
    assert_eq!(json_at(TEXT, "preset.name.en"), None, "没写的");
    assert_eq!(json_at(TEXT, "preset.name"), None, "是表的");
    assert_eq!(json_at(TEXT, "nowhere.at.all"), None);
    assert_eq!(json_at("[broken", "a.b"), None, "读不懂的");
    assert_eq!(json_at("x = [1.5, 2.0]\n", "x"), Some(json!([1.5, 2.0])));
}

#[test]
fn only_scalars_become_values() {
    assert_eq!(from_json(&json!("on")), Some(Value::Text("on".into())));
    assert_eq!(from_json(&json!(true)), Some(Value::Bool(true)));
    assert_eq!(from_json(&json!(3)), Some(Value::Int(3)));
    assert!(matches!(from_json(&json!(1.5)), Some(Value::Float(_))));
    for other in [json!(null), json!([1]), json!({"a": 1})] {
        assert_eq!(from_json(&other), None, "{other}");
    }
}
