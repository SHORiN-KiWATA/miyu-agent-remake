//! `by` 的测试：图纸上的七种读写一字不差；不认识的原样留着；坏的报错。

use super::*;
use crate::test_support::{rejected, round_trip};

#[test]
fn every_kind_from_the_drawing_round_trips() {
    for json in [
        r#"{"kind":"person","account":"alice"}"#,
        r#"{"kind":"external","venue":"qq:group:123456","id":"qq:10086"}"#,
        r#"{"kind":"model","endpoint":"deepseek","model":"deepseek-v4"}"#,
        r#"{"kind":"tool","call_id":"call_44_1"}"#,
        r#"{"kind":"module","id":"memory"}"#,
        r#"{"kind":"session","id":"0192f3a0-1111-7abc-8def-001122334455"}"#,
        r#"{"kind":"kernel"}"#,
    ] {
        round_trip::<By>(json);
        let by: By = serde_json::from_str(json).unwrap();
        assert!(!matches!(by, By::Unknown(_)), "{json} 应该认得出种类");
    }
}

#[test]
fn each_kind_reads_into_its_own_variant() {
    let by: By = serde_json::from_str(r#"{"kind":"person","account":"alice"}"#).unwrap();
    let account = AccountId::parse("alice").unwrap();
    assert_eq!(by, By::Person(Person { account }));
    let by: By = serde_json::from_str(r#"{"kind":"kernel"}"#).unwrap();
    assert_eq!(by, By::Kernel);
}

#[test]
fn unknown_kind_is_kept_byte_for_byte() {
    let json = r#"{"kind":"robot", "serial":"x-1","nested":{"a":[1, 2.50]}}"#;
    let by: By = serde_json::from_str(json).unwrap();
    assert!(matches!(by, By::Unknown(_)), "{by:?}");
    assert_eq!(serde_json::to_string(&by).unwrap(), json);
}

/// 新版本给认识的种类加了字段：原文在日志里留着，读进内存时不认识的字段不管（03 第八节第 1 条）。
#[test]
fn a_known_kind_with_new_fields_reads_the_fields_it_knows() {
    let by: By = serde_json::from_str(r#"{"kind":"module","id":"memory","since":"v2"}"#).unwrap();
    let id = ModuleId::parse("memory").unwrap();
    assert_eq!(by, By::Module(Module { id }));
}

#[test]
fn broken_by_is_an_error() {
    rejected::<By>(r#"{"account":"alice"}"#, "缺了 kind 字段");
    rejected::<By>(r#"{"kind":7}"#, "invalid type");
    rejected::<By>(r#"{"kind":"person","account":"Alice"}"#, "账号的写法不对");
    rejected::<By>(r#"{"kind":"person"}"#, "account");
    rejected::<By>(r#""person""#, "invalid type");
}
