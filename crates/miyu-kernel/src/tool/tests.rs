//! 工具的测试：参数修正的每一种还原；还原不了的不换；字符串不碰；没写的当空对象；
//! 不是对象的报错；参数格式读不了、没有 properties 的原样过；访问类别的写法，哪几类算写入。

use super::*;

const SCHEMA: &str = r#"{"type":"object","properties":{"path":{"type":"string"},"paths":{"type":"array"},"options":{"type":"object"},"limit":{"type":"integer"},"ratio":{"type":"number"},"all":{"type":"boolean"},"loose":{}},"required":["path"]}"#;

fn schema(text: &str) -> RawJson {
    serde_json::from_str(text).unwrap()
}

fn repaired(args: &str) -> String {
    repair(&schema(SCHEMA), args).unwrap()
}

#[test]
fn each_declared_shape_is_restored_from_a_string() {
    for (args, expected) in [
        (r#"{"paths":"[\"a\",\"b\"]"}"#, r#"{"paths":["a","b"]}"#),
        (
            r#"{"options":"{\"deep\":true}"}"#,
            r#"{"options":{"deep":true}}"#,
        ),
        (r#"{"limit":"20"}"#, r#"{"limit":20}"#),
        (r#"{"ratio":" 0.5 "}"#, r#"{"ratio":0.5}"#),
        (r#"{"all":"False"}"#, r#"{"all":false}"#),
    ] {
        assert_eq!(repaired(args), expected, "{args}");
    }
}

#[test]
fn what_cannot_be_restored_or_is_a_string_is_left_alone() {
    for args in [
        r#"{"limit":"twenty"}"#,
        r#"{"ratio":"NaN"}"#,
        r#"{"paths":"a,b"}"#,
        r#"{"options":"[1]"}"#,
        r#"{"all":"yes"}"#,
        r#"{"path":"[\"looks\",\"like\",\"json\"]"}"#,
        r#"{"loose":"20"}"#,
        r#"{"extra":"20"}"#,
        r#"{ "limit": 20 }"#,
    ] {
        assert_eq!(repaired(args), args, "没改的原文照交");
    }
}

#[test]
fn only_changed_arguments_are_rewritten_and_the_rest_are_kept() {
    let out = repaired(r#"{"path":"src","limit":"3"}"#);
    let value: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(value["path"], "src");
    assert_eq!(value["limit"], 3);
}

#[test]
fn empty_arguments_are_an_empty_object() {
    assert_eq!(repaired(""), "{}");
    assert_eq!(repaired("  \n"), "{}");
}

#[test]
fn arguments_that_are_not_an_object_are_refused() {
    for args in ["[1,2]", "null", "\"src\"", "{\"path\":", "20"] {
        assert_eq!(repair(&schema(SCHEMA), args), Err(NotAnObject), "{args}");
    }
}

#[test]
fn a_schema_without_properties_passes_arguments_through() {
    let loose = schema(r#"{"type":"object"}"#);
    assert_eq!(
        repair(&loose, r#"{"limit":"20"}"#).unwrap(),
        r#"{"limit":"20"}"#
    );
}

#[test]
fn access_is_written_as_text_and_unknown_kinds_are_kept() {
    for (text, access) in [
        ("read", Access::Read),
        ("write", Access::Write),
        ("execute", Access::Execute),
        ("network", Access::Network),
        ("outbound", Access::Outbound),
    ] {
        let json = format!("\"{text}\"");
        assert_eq!(serde_json::from_str::<Access>(&json).unwrap(), access);
        assert_eq!(serde_json::to_string(&access).unwrap(), json);
    }
    let clipboard: Access = serde_json::from_str("\"clipboard\"").unwrap();
    assert_eq!(clipboard, Access::Other("clipboard".to_string()));
    assert_eq!(serde_json::to_string(&clipboard).unwrap(), "\"clipboard\"");
}

#[test]
fn writing_files_and_unknown_kinds_count_as_writing() {
    let writes: Vec<bool> = [
        Access::Read,
        Access::Write,
        Access::Execute,
        Access::Network,
        Access::Outbound,
        Access::Other("clipboard".to_string()),
    ]
    .iter()
    .map(Access::writes)
    .collect();
    assert_eq!(writes, [false, true, false, false, false, true]);
}
