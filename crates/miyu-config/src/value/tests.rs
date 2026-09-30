//! 值写成 TOML、JSON；最终值照默认值。

use std::borrow::Cow;

use crate::test_support::item;
use crate::value::{Value, Values};

fn text(text: &str) -> Value {
    Value::Text(Cow::Owned(text.to_string()))
}

#[test]
fn text_is_written_as_a_toml_basic_string() {
    assert_eq!(text("info").toml(), r#""info""#);
    assert_eq!(text("").toml(), r#""""#);
    assert_eq!(text("中文").toml(), r#""中文""#, "别的字照原样");
    assert_eq!(text(r#"a"b\c"#).toml(), r#""a\"b\\c""#, "引号、反斜杠转义");
    assert_eq!(text("a\nb\r\tc").toml(), r#""a\nb\r\tc""#);
    assert_eq!(
        text("\u{1}\u{7f}\u{85}").toml(),
        r#""\u0001\u007F\u0085""#,
        "别的控制字符写成转义"
    );
}

#[test]
fn text_is_a_json_string() {
    assert_eq!(text("a\"b").json(), serde_json::json!("a\"b"));
}

#[test]
fn defaults_hold_every_item_and_nothing_else() {
    let items = [
        item("ui.language", &["auto", "zh"], "auto"),
        item("log.level", &["info", "off"], "off"),
    ];
    let values = Values::defaults(&items);
    assert_eq!(values.get("ui.language"), Some(&text("auto")));
    assert_eq!(values.get("log.level"), Some(&text("off")));
    assert_eq!(values.get("ui"), None);
    assert_eq!(Values::default().get("ui.language"), None);
}

#[test]
fn a_text_setting_is_the_text_itself() {
    assert_eq!(String::from(&text("zh")), "zh");
}
