//! `by` 的测试：图纸上的八种读写一字不差；不认识的原样留着；坏的报错。

use super::*;
use crate::test_support::{rejected, round_trip};

#[test]
fn every_kind_from_the_drawing_round_trips() {
    for json in [
        r#"{"kind":"person","account":"alice"}"#,
        // 施工 O-3：私聊里认出的本人带 `via`；群里的外部身份带对应的账号、桥报的身份。
        r#"{"kind":"person","account":"alice","via":"qq:10001"}"#,
        r#"{"kind":"external","venue":"qq:group:123456","id":"qq:10086"}"#,
        r#"{"kind":"external","venue":"qq:group:123456","id":"qq:10001","account":"alice","role":"manager"}"#,
        r#"{"kind":"external","venue":"qq:group:123456","id":"qq:10086","role":"member"}"#,
        r#"{"kind":"model","endpoint":"deepseek","model":"deepseek-v4"}"#,
        r#"{"kind":"tool","call_id":"call_44_1"}"#,
        r#"{"kind":"module","id":"memory"}"#,
        r#"{"kind":"session","id":"0192f3a0-1111-7abc-8def-001122334455"}"#,
        r#"{"kind":"kernel"}"#,
        r#"{"kind":"harness","name":"claude-code"}"#,
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
    assert_eq!(by, By::Person(Person::new(account)));
    let by: By = serde_json::from_str(r#"{"kind":"kernel"}"#).unwrap();
    assert_eq!(by, By::Kernel);
    let by: By = serde_json::from_str(r#"{"kind":"harness","name":"claude-code"}"#).unwrap();
    let name = HarnessName::parse("claude-code").unwrap();
    assert_eq!(by, By::Harness(Harness { name }));
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
    rejected::<By>(r#"{"account":"alice"}"#, "missing field `kind`");
    rejected::<By>(r#"{"kind":7}"#, "invalid type");
    rejected::<By>(r#"{"kind":"person","account":"Alice"}"#, "bad account");
    rejected::<By>(r#"{"kind":"person"}"#, "account");
    rejected::<By>(r#""person""#, "invalid type");
    // 别的 harness 自己报的名字不可信：照短名字的规则查（施工 7-1）。
    rejected::<By>(r#"{"kind":"harness"}"#, "missing field `name`");
    rejected::<By>(r#"{"kind":"harness","name":""}"#, "bad harness name");
    rejected::<By>(
        r#"{"kind":"harness","name":"claude\u0007code"}"#,
        "control characters",
    );
    rejected::<By>(
        &format!(r#"{{"kind":"harness","name":"{}"}}"#, "h".repeat(129)),
        "128 bytes",
    );
}

/// 桥报的身份只有两种（施工 O-3）：没有 `owner`，主人照对应表认。
#[test]
fn a_role_other_than_manager_or_member_is_refused() {
    let read = serde_json::from_str::<By>(
        r#"{"kind":"external","venue":"qq:group:1","id":"qq:1","role":"owner"}"#,
    );
    assert!(read.is_err(), "{read:?}");
}

/// 主人本人（施工 O-2 下）：本机的人、私聊里对应表认出的本人、群里对应表里有的外部身份；别的都不是。
#[test]
fn the_owner_is_a_person_or_someone_on_the_owner_table() {
    let by = |json: &str| serde_json::from_str::<By>(json).unwrap();
    assert!(by(r#"{"kind":"person","account":"alice"}"#).is_owner());
    assert!(
        by(r#"{"kind":"external","venue":"qq:group:1","id":"qq:1","account":"alice"}"#).is_owner()
    );
    assert!(
        !by(r#"{"kind":"external","venue":"qq:group:1","id":"qq:2","role":"manager"}"#).is_owner()
    );
    assert!(!By::Kernel.is_owner());
    assert!(!by(r#"{"kind":"harness","name":"claude"}"#).is_owner());
}
