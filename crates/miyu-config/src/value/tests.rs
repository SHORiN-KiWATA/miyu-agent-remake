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

/// 网址类型用 `String`（不是 [`crate::Address`]）的字段，写成了引用（施工 8-6b：`Kind::Url` 整体认 `{ env = … }`）：
/// 读成空字，不读成那一句 TOML 字节（`models.catalog.url` 这类没有引用的必要，防着悄悄被填进读不出地址的乱码）。
#[test]
fn a_reference_in_a_plain_string_field_reads_as_empty_not_as_toml() {
    use crate::secret::Reference;
    assert_eq!(
        String::from(&Value::Secret(Reference::Env("X".to_string()))),
        ""
    );
}

/// 有默认值的网址（施工 8-8，`models.catalog.url`）：写死的、引用的各读各的，没有的照默认值。
#[test]
fn an_address_with_a_default_reads_a_literal_or_a_reference() {
    use crate::secret::Reference;
    use crate::{Address, Setting};
    assert_eq!(
        Address::read(Some(&text("https://a.invalid"))),
        Address::Literal("https://a.invalid".to_string())
    );
    assert_eq!(
        Address::read(Some(&Value::Secret(Reference::Env("CAT_URL".to_string())))),
        Address::Env("CAT_URL".to_string())
    );
    assert_eq!(Address::read(None), Address::Literal(String::new()));
}
