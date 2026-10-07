//! 报错：离得最近的键名（太远的不给、一样近的取前面的、换位算一次），收到的原文截到 80 个字符，每一种原因码说成
//! 一句话（拿假的中文字，样子照 `config.md`「样子」的例子），段和段怎么接。

use std::borrow::Cow;

use crate::item::{Item, Kind, Layer, Tighten};
use crate::merge::Origin;
use crate::problem::{At, Code, Problem, Severity, Told, Using, got, nearest, tell};
use crate::test_support::{Fake, item, system_only, words};
use crate::value::Value;

fn items() -> Vec<Item> {
    vec![
        item("ui.language", &["auto", "zh", "en", "ja"], "auto"),
        system_only(item(
            "log.level",
            &["error", "warn", "info", "debug", "trace", "off"],
            "info",
        )),
        Item {
            kind: Kind::Bool,
            default: Some(Value::Bool(false)),
            layers: &[Layer::System, Layer::Personal, Layer::Project],
            tighten: Some(Tighten::TrueOnly),
            ..item("permission.start_read_only", &[], "")
        },
    ]
}

fn text(text: &'static str) -> Value {
    Value::Text(Cow::Borrowed(text))
}

fn at(line: usize, column: usize) -> At {
    At { line, column }
}

/// 照假的中文字说成话。
fn said(problem: &Problem, using: Option<&Using>) -> Told {
    tell(problem, &items(), using, &words(&items())).expect("字齐全")
}

/// 清单里离得最近的键：照配置的调法，交清单每一项的键。
fn near(items: &[Item], key: &str) -> Option<&'static str> {
    nearest(items.iter().map(|item| item.key), key)
}

#[test]
fn the_nearest_key_is_close_enough_and_first_on_ties() {
    let items = items();
    assert_eq!(
        near(&items, "ui.langauge"),
        Some("ui.language"),
        "换位算一次"
    );
    assert_eq!(near(&items, "ui.languag"), Some("ui.language"));
    assert_eq!(near(&items, "log.levle"), Some("log.level"));
    assert_eq!(near(&items, "UI.LANGUAGE"), None, "差太多");
    assert_eq!(near(&items, "ui.x"), None);
    assert_eq!(
        near(&items, "lo.lev"),
        None,
        "差 3 个字，超过这个键长的三分之一"
    );
    let short = [item("ab.cd", &["x", "y"], "x")];
    assert_eq!(
        near(&short, "ba.cd"),
        Some("ab.cd"),
        "换位算一次：短键上差两个字就太远了"
    );
    let tied = [
        item("a.bc", &["x", "y"], "x"),
        item("a.bd", &["x", "y"], "x"),
    ];
    assert_eq!(near(&tied, "a.bb"), Some("a.bc"), "一样近的取前面的");
}

#[test]
fn got_is_cut_at_eighty_characters() {
    assert_eq!(got("abc"), "abc");
    assert_eq!(got(&"x".repeat(80)), "x".repeat(80));
    assert_eq!(got(&"界".repeat(81)), format!("{}…", "界".repeat(80)));
}

#[test]
fn positions_count_lines_and_characters() {
    assert_eq!(At::of("a\nbc = 1", 3), at(2, 2));
    assert_eq!(At::of("中文\r\nx", 8), at(2, 1));
    assert_eq!(At::of("中x", 3), at(1, 2));
}

#[test]
fn only_unknown_keys_and_untrusted_projects_are_warnings() {
    assert_eq!(Code::UnknownKey.severity(), Severity::Warning);
    assert_eq!(Code::UntrustedProject.severity(), Severity::Warning);
    assert_eq!(Code::Syntax.severity(), Severity::Error);
    assert_eq!(Severity::Warning.as_str(), "warning");
    assert!(Code::TooBig.whole_file() && !Code::WrongType.whole_file());
    assert_eq!(Code::NotTightening.as_str(), "not_tightening");
}

#[test]
fn an_unknown_key_suggests_and_keeps_the_line() {
    let mut problem = Problem::item(
        Code::UnknownKey,
        Layer::Personal,
        "ui.langauge",
        at(7, 1),
        "\"en\"",
    );
    problem.suggest = Some("ui.language");
    assert_eq!(
        said(&problem, None).message,
        "没有 ui.langauge 这一项。是不是想写 ui.language？这一行先不管，原样留着。"
    );
    problem.suggest = None;
    assert_eq!(
        said(&problem, None).message,
        "没有 ui.langauge 这一项。这一行先不管，原样留着。"
    );
    problem.at = None;
    assert_eq!(
        said(&problem, None).message,
        "没有 ui.langauge 这一项。",
        "查询里的键不在哪一行"
    );
}

#[test]
fn a_bad_option_lists_the_options_and_says_what_is_used() {
    let problem = Problem::item(
        Code::NotAnOption,
        Layer::System,
        "log.level",
        at(2, 9),
        "\"verbose\"",
    );
    let using = Using::Value(text("info"), Origin::Default);
    assert_eq!(
        said(&problem, Some(&using)),
        Told {
            expected: Some("error、warn、info、debug、trace 或 off".to_string()),
            message: "log.level 只能是 error、warn、info、debug、trace 或 off，写的是 \"verbose\"。改成其中一个，\
                      例如 log.level = \"info\"。这一项先照 \"info\" 用着（默认值）。"
                .to_string(),
        }
    );
    let wrong_type = Problem {
        code: Code::WrongType,
        ..problem
    };
    assert_eq!(
        said(&wrong_type, None).message,
        "log.level 只能是 error、warn、info、debug、trace 或 off，写的是 \"verbose\"。改成其中一个，\
         例如 log.level = \"info\"。",
        "选项写成了别的类型，也列出能写的"
    );
}

#[test]
fn a_bad_switch_expects_true_or_false_and_offers_the_other_one() {
    let problem = Problem::item(
        Code::WrongType,
        Layer::Personal,
        "permission.start_read_only",
        at(5, 19),
        "\"yes\"",
    );
    let using = Using::Value(
        Value::Bool(true),
        Origin::File {
            layer: Layer::System,
            line: 2,
        },
    );
    assert_eq!(
        said(&problem, Some(&using)),
        Told {
            expected: Some("true 或 false".to_string()),
            message: "permission.start_read_only 要写 true 或 false，写的是 \"yes\"。\
                      改成 permission.start_read_only = true。这一项先照 true 用着（系统配置）。"
                .to_string(),
        }
    );
}

#[test]
fn a_group_written_as_a_value_expects_a_table() {
    let problem = Problem::item(Code::WrongType, Layer::Personal, "ui", at(1, 6), "\"zh\"");
    let told = said(&problem, Some(&Using::Nothing));
    assert_eq!(told.expected.as_deref(), Some("一张表"));
    assert_eq!(
        told.message, "ui 要写 一张表，写的是 \"zh\"。",
        "不说照什么用"
    );
}

#[test]
fn the_wrong_layer_says_where_to_move_it() {
    let problem = Problem::item(
        Code::WrongLayer,
        Layer::Personal,
        "log.level",
        at(9, 9),
        "\"debug\"",
    );
    let using = Using::Value(text("info"), Origin::Default);
    assert_eq!(
        said(&problem, Some(&using)).message,
        "log.level 只能写在系统配置里，写在个人设置里不算。挪到系统配置里去。"
    );
    let project = Problem::item(
        Code::WrongLayer,
        Layer::Project,
        "ui.language",
        at(1, 1),
        "\"zh\"",
    );
    assert_eq!(
        said(&project, None).message,
        "ui.language 只能写在系统配置或个人设置里，写在项目配置里不算。挪到系统配置或个人设置里去。"
    );
}

#[test]
fn a_looser_project_value_says_what_is_in_force() {
    let mut problem = Problem::item(
        Code::NotTightening,
        Layer::Project,
        "permission.start_read_only",
        at(3, 19),
        "false",
    );
    problem.current = Some(Value::Bool(true));
    assert_eq!(
        said(&problem, None).message,
        "项目配置只能让限制更严。permission.start_read_only 现在是 true，这里写的 false 更宽，不算。"
    );
}

#[test]
fn whole_file_problems_say_what_the_file_is_used_as() {
    let syntax = Problem {
        at: Some(at(4, 12)),
        ..Problem::file(
            Code::Syntax,
            Layer::Personal,
            Some("invalid basic string".to_string()),
        )
    };
    assert_eq!(
        said(&syntax, Some(&Using::LastGood)).message,
        "TOML 写法不对：invalid basic string。这份文件先照上一次读进来的用着。"
    );
    let unreadable = Problem::file(
        Code::Unreadable,
        Layer::System,
        Some("Permission denied".to_string()),
    );
    assert_eq!(
        said(&unreadable, Some(&Using::Nothing)).message,
        "读不了这份文件：Permission denied。这份文件先不用。"
    );
    let big = Problem::file(Code::TooBig, Layer::System, None);
    assert_eq!(said(&big, None).message, "这份文件超过 1 MiB，不读。");
    let utf8 = Problem::file(Code::NotUtf8, Layer::System, None);
    assert_eq!(
        said(&utf8, Some(&Using::LastGood)).message,
        "这份文件不是 UTF-8。这份文件先照上一次读进来的用着。"
    );
    let untrusted = Problem::file(Code::UntrustedProject, Layer::Project, None);
    assert_eq!(
        said(&untrusted, None).message,
        "这份项目配置还没信任，先不用。"
    );
}

#[test]
fn english_joins_with_a_space_and_a_full_stop() {
    let mut english: Fake = words(&items());
    for (key, sentence) in [
        (
            "config/unknown-key",
            "There is no {key}. Did you mean {suggest}?",
        ),
        ("config/kept", "The line is ignored and kept as it is"),
        ("config/sentence", "{text}."),
        ("config/then", "{rest} {next}"),
        ("config/stops", ".?!"),
    ] {
        english
            .sentences
            .insert(key.to_string(), sentence.to_string());
    }
    let mut problem = Problem::item(
        Code::UnknownKey,
        Layer::Personal,
        "ui.langauge",
        at(7, 1),
        "1",
    );
    problem.suggest = Some("ui.language");
    assert_eq!(
        tell(&problem, &items(), None, &english)
            .expect("字齐全")
            .message,
        "There is no ui.langauge. Did you mean ui.language? The line is ignored and kept as it is."
    );
}

#[test]
fn a_missing_sentence_is_named() {
    let mut fake = words(&items());
    fake.sentences.remove("config/kept");
    let problem = Problem::item(Code::UnknownKey, Layer::Personal, "x.y", at(1, 1), "1");
    let missing = tell(&problem, &items(), None, &fake).expect_err("缺字");
    assert_eq!(missing.to_string(), "no words for config/kept");
}
