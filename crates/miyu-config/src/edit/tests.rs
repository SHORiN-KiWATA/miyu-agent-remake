//! 改一项、加一项（有表、没表、点号连着的、行内表）、删一项（表空了连表头删、有注释的留），只动那一项（前后字节比），
//! 换行照原文件，新文件第一行是 `#:schema`，放不进去的报出来；`input`、JSON 的值照类型读。

use std::borrow::Cow;

use crate::edit::{Blocked, Change, apply, from_json, input, new_file};
use crate::item::Kind;
use crate::value::Value;

fn zh() -> Value {
    Value::Text(Cow::Borrowed("zh"))
}

fn set(text: &str, key: &str, value: &Value) -> String {
    apply(text, Change::Set(key, value)).expect("放得进去")
}

fn unset(text: &str, key: &str) -> String {
    apply(text, Change::Unset(key)).expect("删得掉")
}

#[test]
fn a_value_is_changed_in_place_and_nothing_else_moves() {
    let text = "# 我的设置\n\n[ui]\nlanguage   =  \"en\"   # 自己改的\nlangauge = 1\n\n[log]\nlevel = \"info\"\n";
    assert_eq!(
        set(text, "ui.language", &zh()),
        "# 我的设置\n\n[ui]\nlanguage   =  \"zh\"   # 自己改的\nlangauge = 1\n\n[log]\nlevel = \"info\"\n",
        "注释、空格、不认识的键都照原样"
    );
    let on = Value::Bool(true);
    assert_eq!(
        set(
            "[permission]\nstart_read_only = false\n",
            "permission.start_read_only",
            &on
        ),
        "[permission]\nstart_read_only = true\n"
    );
}

#[test]
fn a_new_key_goes_after_the_last_key_of_its_table() {
    let text = "[ui]\n# 先放着\ntheme = \"dark\"\n# 表尾的注释\n\n[log]\nlevel = \"info\"\n";
    assert_eq!(
        set(text, "ui.language", &zh()),
        "[ui]\n# 先放着\ntheme = \"dark\"\nlanguage = \"zh\"\n# 表尾的注释\n\n[log]\nlevel = \"info\"\n"
    );
    let empty = "[ui] # 空的\n\n[log]\nlevel = \"info\"\n";
    assert_eq!(
        set(empty, "ui.language", &zh()),
        "[ui] # 空的\nlanguage = \"zh\"\n\n[log]\nlevel = \"info\"\n",
        "表里还没有键的，接在表头后面"
    );
}

#[test]
fn a_missing_table_is_opened_at_the_end_after_a_blank_line() {
    assert_eq!(
        set("[log]\nlevel = \"info\"\n", "ui.language", &zh()),
        "[log]\nlevel = \"info\"\n\n[ui]\nlanguage = \"zh\"\n"
    );
    assert_eq!(
        set("[log]\nlevel = \"info\"", "ui.language", &zh()),
        "[log]\nlevel = \"info\"\n\n[ui]\nlanguage = \"zh\"\n",
        "最后一行没有换行的先补上"
    );
    assert_eq!(
        set("[log]\nlevel = \"info\"\n\n", "ui.language", &zh()),
        "[log]\nlevel = \"info\"\n\n[ui]\nlanguage = \"zh\"\n",
        "已经空着一行的不再空"
    );
    assert_eq!(set("", "ui.language", &zh()), "[ui]\nlanguage = \"zh\"\n");
    assert_eq!(
        set("[ui.theme]\nname = 1\n", "ui.language", &zh()),
        "[ui.theme]\nname = 1\n\n[ui]\nlanguage = \"zh\"\n",
        "只因为子表才有的表没有表头：开一张"
    );
}

#[test]
fn dotted_keys_and_inline_tables_are_written_the_way_they_already_are() {
    assert_eq!(
        set("ui.langauge = \"x\"\n", "ui.language", &zh()),
        "ui.langauge = \"x\"\nui.language = \"zh\"\n"
    );
    assert_eq!(
        set("ui.language = \"en\" # 点号\n", "ui.language", &zh()),
        "ui.language = \"zh\" # 点号\n"
    );
    assert_eq!(
        set("ui = { theme = \"dark\" }\n", "ui.language", &zh()),
        "ui = { theme = \"dark\", language = \"zh\" }\n"
    );
    assert_eq!(
        set("ui = {}\n", "ui.language", &zh()),
        "ui = { language = \"zh\" }\n"
    );
    assert_eq!(
        unset(
            "ui = { language = \"en\", theme = \"dark\" }\n",
            "ui.language"
        ),
        "ui = { theme = \"dark\" }\n"
    );
    assert_eq!(
        unset(
            "ui = { theme = \"dark\", language = \"en\" }\n",
            "ui.language"
        ),
        "ui = { theme = \"dark\" }\n"
    );
    assert_eq!(
        unset("ui.language = \"en\"\nx = 1\n", "ui.language"),
        "x = 1\n"
    );
}

#[test]
fn a_removed_key_takes_its_line_and_an_emptied_table_its_header() {
    let text = "[ui]\nlanguage = \"en\" # 行尾\ntheme = 1\n";
    assert_eq!(unset(text, "ui.language"), "[ui]\ntheme = 1\n");
    let middle = "[log]\nlevel = \"info\"\n\n[ui]\nlanguage = \"en\"\n\n[permission]\nstart_read_only = true\n";
    assert_eq!(
        unset(middle, "ui.language"),
        "[log]\nlevel = \"info\"\n\n[permission]\nstart_read_only = true\n",
        "表空了连表头删，前后几张表不动"
    );
    let commented = "[ui]\n# 以后再定\nlanguage = \"en\"\n";
    assert_eq!(
        unset(commented, "ui.language"),
        "[ui]\n# 以后再定\n",
        "里面有注释的，表头留着"
    );
    let unknown = "[ui]\nlanguage = \"en\"\nweird = 1\n";
    assert_eq!(unset(unknown, "ui.language"), "[ui]\nweird = 1\n");
    assert_eq!(
        unset("[ui]\ntheme = 1\n", "ui.language"),
        "[ui]\ntheme = 1\n",
        "没写的删了还是原样"
    );
}

#[test]
fn a_new_file_starts_with_the_schema_line_and_set_then_unset_goes_back() {
    let fresh = new_file("../state/config/config.schema.json");
    assert_eq!(fresh, "#:schema ../state/config/config.schema.json\n");
    let written = set(&fresh, "ui.language", &zh());
    assert_eq!(
        written,
        "#:schema ../state/config/config.schema.json\n\n[ui]\nlanguage = \"zh\"\n"
    );
    assert_eq!(unset(&written, "ui.language"), fresh, "改了再删回到原样");
}

#[test]
fn line_endings_follow_the_file() {
    let text = "[ui]\r\nlanguage = \"en\"\r\n";
    assert_eq!(
        set(text, "log.level", &Value::Text(Cow::Borrowed("debug"))),
        "[ui]\r\nlanguage = \"en\"\r\n\r\n[log]\r\nlevel = \"debug\"\r\n"
    );
    assert_eq!(
        set(text, "ui.theme", &zh()),
        "[ui]\r\nlanguage = \"en\"\r\ntheme = \"zh\"\r\n"
    );
    assert_eq!(unset(text, "ui.language"), "");
}

#[test]
fn values_are_written_as_toml_strings() {
    let odd = Value::Text(Cow::Borrowed("a\"b\\c"));
    assert_eq!(
        set("[ui]\nlanguage = \"en\"\n", "ui.language", &odd),
        "[ui]\nlanguage = \"a\\\"b\\\\c\"\n"
    );
}

#[test]
fn what_cannot_be_placed_is_blocked() {
    for text in [
        "ui = \"zh\"\n",
        "[[ui]]\nlanguage = \"en\"\n",
        "[ui.language]\nx = 1\n",
        "ui = [1]\n",
    ] {
        assert_eq!(
            apply(text, Change::Set("ui.language", &zh())),
            Err(Blocked("ui.language".to_string())),
            "{text:?}"
        );
    }
    assert_eq!(
        apply("[ui\n", Change::Set("ui.language", &zh())),
        Err(Blocked("ui.language".to_string())),
        "读不懂的字不改"
    );
}

#[test]
fn input_is_read_by_the_kind() {
    let options = Kind::Option(&["zh", "en"]);
    assert_eq!(input(Kind::Bool, "true"), Some(Value::Bool(true)));
    assert_eq!(input(Kind::Bool, "false"), Some(Value::Bool(false)));
    assert_eq!(input(Kind::Bool, "yes"), None);
    assert_eq!(input(Kind::Bool, "True"), None, "开关只认小写的");
    assert_eq!(input(options, "zh"), Some(zh()));
    assert_eq!(input(options, "\"zh\""), Some(zh()), "两头的双引号去掉");
    assert_eq!(
        input(options, "\"a\\\"b\""),
        Some(Value::Text(Cow::Borrowed("a\"b"))),
        "照 TOML 转义读"
    );
    assert_eq!(
        input(options, "\"a\"\nb = \"c\""),
        Some(Value::Text(Cow::Borrowed("\"a\"\nb = \"c\""))),
        "不是一个 TOML 字符串的照原样"
    );
    assert_eq!(input(options, "fr"), Some(Value::Text(Cow::Borrowed("fr"))));
}

#[test]
fn json_values_are_read_by_the_kind() {
    let options = Kind::Option(&["zh", "en"]);
    assert_eq!(from_json(options, &serde_json::json!("zh")), Some(zh()));
    assert_eq!(from_json(options, &serde_json::json!(3)), None);
    assert_eq!(
        from_json(Kind::Bool, &serde_json::json!(true)),
        Some(Value::Bool(true))
    );
    assert_eq!(from_json(Kind::Bool, &serde_json::json!("true")), None);
    assert_eq!(from_json(Kind::Bool, &serde_json::Value::Null), None);
}
