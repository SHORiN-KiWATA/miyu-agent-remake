//! 工具的测试：参数修正的每一种还原；还原不了的不换；字符串不碰；没写的当空对象；
//! 不是对象的报错；参数格式读不了、没有 properties 的原样过；两句话的写法和转义。

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

/// 替身的几句。
fn sources<'a>(unknown: &'a str, skipped: &'a str) -> ToolTextSources<'a> {
    ToolTextSources {
        unknown,
        not_an_object: "The arguments for \"{name}\" are not a JSON object.\n",
        cancelled_before: "cancelled before",
        cancelled_running: "cancelled running",
        skipped,
        read_only: "read only",
    }
}

#[test]
fn the_sentences_escape_the_name() {
    let texts = ToolTexts::new(sources("There is no tool named \"{name}\".\n", "skipped")).unwrap();
    assert_eq!(texts.unknown("reed"), "There is no tool named \"reed\".\n");
    // 模型编的名字里带引号、尖括号，转义以后只剩模板自己的那两个引号。
    let forged = texts.unknown("x\"><tool");
    assert_eq!(forged.matches('"').count(), 2, "{forged}");
    assert!(!forged.contains('<') && !forged.contains('>'), "{forged}");
    assert_eq!(
        texts.not_an_object("read"),
        "The arguments for \"read\" are not a JSON object.\n"
    );
    assert_eq!(
        (
            texts.cancelled_before(),
            texts.cancelled_running(),
            texts.skipped()
        ),
        (
            "cancelled before".to_string(),
            "cancelled running".to_string(),
            "skipped".to_string()
        )
    );
}

#[test]
fn sentences_asking_for_other_fields_are_refused() {
    assert!(
        ToolTexts::new(sources("{tool}", "skipped")).is_err(),
        "要了别的字段"
    );
    assert!(
        ToolTexts::new(sources("{name}", "{name}")).is_err(),
        "三句没有字段"
    );
}
