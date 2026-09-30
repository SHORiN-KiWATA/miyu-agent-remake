//! 读一份配置：每一种原因码各一例，行、列、`got` 截到 80 个字符；BOM；`\r\n`；`toml_edit` 的报错只取为什么那一行；
//! 键名拼错给最近的、太远的不给、一样近的取前面的。

use std::borrow::Cow;

use crate::item::{Item, Kind, Layer, Tighten};
use crate::parse::{Entry, parse};
use crate::problem::{At, Code, Problem};
use crate::test_support::{item, system_only};
use crate::value::Value;

/// 手写的清单：一个选项（系统、个人），一个只能放系统配置的选项，一个三层都能放的开关。
fn items() -> Vec<Item> {
    vec![
        item("ui.language", &["auto", "zh", "en"], "auto"),
        system_only(item("log.level", &["info", "debug"], "info")),
        Item {
            kind: Kind::Bool,
            default: Value::Bool(false),
            layers: &[Layer::System, Layer::Personal, Layer::Project],
            tighten: Some(Tighten::TrueOnly),
            ..item("permission.start_read_only", &[], "")
        },
    ]
}

fn at(line: usize, column: usize) -> At {
    At { line, column }
}

fn text(text: &str) -> Value {
    Value::Text(Cow::Owned(text.to_string()))
}

/// 一条问题看的几样：原因码、键、位置、收到的。
type Seen = (Code, Option<String>, Option<At>, Option<String>);

/// 当成个人设置读，问题只看原因码、键、位置、收到的。
fn problems(source: &str) -> Vec<Seen> {
    parse(&items(), Layer::Personal, source)
        .expect("写法对")
        .problems
        .into_iter()
        .map(|problem| (problem.code, problem.key, problem.at, problem.got))
        .collect()
}

fn one(
    code: Code,
    key: &str,
    at: At,
    got: &str,
) -> (Code, Option<String>, Option<At>, Option<String>) {
    (code, Some(key.to_string()), Some(at), Some(got.to_string()))
}

#[test]
fn good_entries_remember_their_line() {
    let parsed = parse(
        &items(),
        Layer::Personal,
        "# 注释\n\n[ui]\nlanguage = \"zh\" # 行尾\n\n[permission]\nstart_read_only = true\n",
    )
    .expect("写法对");
    assert_eq!(parsed.problems, Vec::<Problem>::new());
    assert_eq!(
        parsed.entries.get("ui.language"),
        Some(&Entry {
            value: text("zh"),
            line: 4,
            at: at(4, 12),
            raw: "\"zh\"".to_string(),
            counts: true,
        })
    );
    assert_eq!(
        parsed.entries["permission.start_read_only"].value,
        Value::Bool(true)
    );
    assert_eq!(parsed.entries["permission.start_read_only"].line, 7);
}

#[test]
fn dotted_keys_and_inline_tables_are_the_same_items() {
    let parsed = parse(
        &items(),
        Layer::Personal,
        "ui.language = \"en\"\npermission = { start_read_only = true }\n",
    )
    .expect("写法对");
    assert_eq!(parsed.problems, Vec::<Problem>::new());
    assert_eq!(parsed.entries["ui.language"].line, 1);
    assert_eq!(parsed.entries["permission.start_read_only"].line, 2);
}

#[test]
fn an_unknown_key_warns_and_suggests_the_nearest() {
    // 不认识的一张表往里走，每一项照整个键报；空的不认识的表没什么可报。
    let source = "[ui]\nlanguage = \"zh\"\nlangauge = \"en\"\n\n[weird]\nthing = 1\n[empty]\nuii.language = 1\n";
    let parsed = parse(&items(), Layer::Personal, source).expect("写法对");
    assert_eq!(parsed.entries.len(), 1, "写对的照收");
    let found: Vec<_> = parsed
        .problems
        .iter()
        .map(|problem| {
            (
                problem.code,
                problem.key.as_deref(),
                problem.at,
                problem.suggest,
            )
        })
        .collect();
    assert_eq!(
        found,
        [
            (
                Code::UnknownKey,
                Some("ui.langauge"),
                Some(at(3, 1)),
                Some("ui.language")
            ),
            (Code::UnknownKey, Some("weird.thing"), Some(at(6, 1)), None),
            (
                Code::UnknownKey,
                Some("empty.uii.language"),
                Some(at(8, 1)),
                None
            ),
        ]
    );
    assert_eq!(
        parsed.problems[0].code.severity(),
        crate::problem::Severity::Warning
    );
}

#[test]
fn a_wrong_type_drops_only_that_item() {
    assert_eq!(
        problems("[ui]\nlanguage = \"zh\"\n[permission]\nstart_read_only = \"yes\"\n"),
        [one(
            Code::WrongType,
            "permission.start_read_only",
            at(4, 19),
            "\"yes\""
        )]
    );
    let parsed = parse(&items(), Layer::Personal, "ui.language = 3\n").expect("写法对");
    assert!(parsed.entries.is_empty());
    assert_eq!(parsed.problems[0].code, Code::WrongType, "选项写成数");
}

#[test]
fn a_value_where_a_table_belongs_is_a_wrong_type() {
    assert_eq!(
        problems("ui = \"zh\"\n"),
        [one(Code::WrongType, "ui", at(1, 6), "\"zh\"")]
    );
}

#[test]
fn a_table_where_a_value_belongs_is_a_wrong_type() {
    assert_eq!(
        problems("[ui.language]\nx = 1\n"),
        [one(
            Code::WrongType,
            "ui.language",
            at(1, 5),
            "[ui.language]"
        )]
    );
}

#[test]
fn an_option_must_be_listed_and_is_case_sensitive() {
    assert_eq!(
        problems("[ui]\nlanguage = \"ZH\"\n"),
        [one(Code::NotAnOption, "ui.language", at(2, 12), "\"ZH\"")]
    );
}

#[test]
fn a_key_in_the_wrong_layer_does_not_count_but_is_remembered() {
    let parsed = parse(&items(), Layer::Personal, "[log]\nlevel = \"debug\"\n").expect("写法对");
    assert_eq!(
        parsed
            .problems
            .iter()
            .map(|p| (p.code, p.at))
            .collect::<Vec<_>>(),
        [(Code::WrongLayer, Some(at(2, 9)))]
    );
    assert!(!parsed.entries["log.level"].counts, "不算，只记着");
    let parsed = parse(&items(), Layer::Project, "[ui]\nlanguage = 3\n").expect("写法对");
    assert_eq!(
        parsed.problems.iter().map(|p| p.code).collect::<Vec<_>>(),
        [Code::WrongLayer],
        "层不对的只报层，不再查类型"
    );
    assert!(parsed.entries.is_empty());
    let parsed = parse(&items(), Layer::System, "[log]\nlevel = \"debug\"\n").expect("写法对");
    assert!(parsed.problems.is_empty() && parsed.entries["log.level"].counts);
}

#[test]
fn bad_toml_is_one_syntax_problem_with_only_the_reason() {
    let problem =
        parse(&items(), Layer::Personal, "[ui]\nlanguage = \"zh\n").expect_err("写法不对");
    assert_eq!(problem.code, Code::Syntax);
    assert_eq!(
        problem.at,
        Some(at(2, 15)),
        "照 toml_edit 报的位置：字没收尾的那一行行尾"
    );
    let why = problem.why.expect("有为什么");
    assert!(
        !why.contains('\n') && !why.contains("language"),
        "只取为什么：{why}"
    );
    assert!(!why.is_empty());
    assert_eq!(problem.key, None);
    assert_eq!(problem.got, None, "不带原文");
}

#[test]
fn got_is_cut_at_eighty_characters() {
    let long = "字".repeat(100);
    let found = problems(&format!("[ui]\nlanguage = \"{long}\"\n"));
    let got = found[0].3.as_deref().expect("有原文");
    assert_eq!(got.chars().count(), 81, "80 个字加 …");
    assert!(got.starts_with("\"字") && got.ends_with('…'));
    let exact = format!("\"{}\"", "a".repeat(78));
    assert_eq!(
        problems(&format!("ui.language = {exact}\n"))[0]
            .3
            .as_deref(),
        Some(exact.as_str()),
        "正好 80 个的不截"
    );
}

#[test]
fn a_bom_and_crlf_do_not_move_lines_or_columns() {
    let parsed = parse(
        &items(),
        Layer::Personal,
        "\u{FEFF}[ui]\r\nlanguage = \"zh\"\r\n[permission]\r\nstart_read_only = 1\r\n",
    )
    .expect("写法对");
    assert_eq!(parsed.entries["ui.language"].line, 2);
    assert_eq!(parsed.entries["ui.language"].at, at(2, 12));
    assert_eq!(parsed.problems[0].at, Some(at(4, 19)));
    let first = parse(&items(), Layer::Personal, "\u{FEFF}ui.language = 3\n").expect("写法对");
    assert_eq!(
        first.problems[0].at,
        Some(at(1, 15)),
        "BOM 不算第一行的一列"
    );
}

#[test]
fn columns_count_characters_not_bytes() {
    assert_eq!(
        problems("# 中文注释\n\"界面\" = 1\nui.language = \"中\" \n")[0].2,
        Some(at(2, 1))
    );
    let parsed = parse(&items(), Layer::Personal, "ui = { \"界\" = 1, x = 1 }\n").expect("写法对");
    assert_eq!(
        parsed
            .problems
            .iter()
            .map(|p| (p.key.as_deref(), p.at))
            .collect::<Vec<_>>(),
        [
            (Some("ui.界"), Some(at(1, 8))),
            (Some("ui.x"), Some(at(1, 17)))
        ],
        "中文算一列"
    );
}

#[test]
fn a_misspelt_group_still_finds_the_nearest_key() {
    let parsed = parse(&items(), Layer::Personal, "uii.language = \"zh\"\n").expect("写法对");
    assert_eq!(
        parsed
            .problems
            .iter()
            .map(|p| (p.key.as_deref(), p.suggest))
            .collect::<Vec<_>>(),
        [(Some("uii.language"), Some("ui.language"))]
    );
}
