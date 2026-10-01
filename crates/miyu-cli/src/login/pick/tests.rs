//! 列出、编号表照显示的宽度对齐，设了的在前、没设的灰；敲的编号、新名字、写错的、直接回车各认成什么。

use serde_json::json;

use super::*;

fn secrets() -> Vec<Value> {
    vec![
        json!({"name": "bigmodel-2", "set": false, "used_by": ["providers.bigmodel.keys"]}),
        json!({"name": "deepseek", "set": true, "used_by": ["providers.deepseek.keys"]}),
        json!({"name": "zeta", "set": true, "used_by": []}),
    ]
}

#[test]
fn the_list_puts_set_keys_first_and_aligns_columns() {
    let lines: Vec<String> = table(&secrets(), Language::Chinese)
        .iter()
        .map(|line| line.paint(false))
        .collect();
    assert_eq!(
        lines,
        [
            "deepseek    已设置  providers.deepseek.keys\n",
            "zeta        已设置\n",
            "bigmodel-2  未设置  providers.bigmodel.keys\n",
        ]
    );
    let painted = table(&secrets(), Language::English)[2].paint(true);
    assert!(
        painted.starts_with("\u{1b}[") && painted.contains("not set"),
        "{painted:?}"
    );
}

#[test]
fn the_numbered_table_follows_the_sample() {
    let lines: Vec<String> = numbered(&secrets()[..2], Language::Chinese)
        .iter()
        .map(|line| line.paint(false))
        .collect();
    assert_eq!(
        lines,
        [
            "  1  bigmodel-2  providers.bigmodel.keys  未设置\n",
            "  2  deepseek    providers.deepseek.keys  已设置\n",
        ]
    );
}

#[test]
fn typed_words_pick_a_number_a_new_name_or_nothing() {
    let secrets = secrets();
    assert_eq!(chosen("2", &secrets), Chosen::Name("deepseek".to_string()));
    assert_eq!(
        chosen("new-one", &secrets),
        Chosen::Name("new-one".to_string())
    );
    assert_eq!(chosen("", &secrets), Chosen::Nothing);
    assert_eq!(
        chosen("9", &secrets),
        Chosen::Bad("9".to_string()),
        "超出编号的不是名字"
    );
    assert_eq!(chosen("0", &secrets), Chosen::Bad("0".to_string()));
    assert_eq!(
        chosen("Deep Seek", &secrets),
        Chosen::Bad("Deep Seek".to_string())
    );
}
