//! 密钥（施工 8-5）：名字的写法；引用的两种写法认得出、别的不认；密钥不会被印出来；密钥文件的字：写错的一行报问题、
//! 不带收到的原文，TOML 写错时报的话里没有密钥；改一行只动那一行；引用的密钥、环境变量没设的报警告；类型是密钥的一项
//! 读、说、生成 Schema。

use std::collections::BTreeSet;

use crate::item::{Item, Kind, Layer};
use crate::merge::{Layers, merge};
use crate::parse::parse;
use crate::problem::{At, Code, Severity, tell};
use crate::secret::{
    Reference, Refused, Secret, VALUE_BYTES, missing, parse_file, set_in, unset_in, valid_name,
};
use crate::test_support::{item, secret_item, words};
use crate::value::Value;

/// 一眼看得出是假的 key。
const FAKE: &str = "sk-FAKE-KEY-FOR-TESTS-0001";
const FAKE_2: &str = "sk-FAKE-KEY-FOR-TESTS-0002";

fn items() -> Vec<Item> {
    vec![
        item("ui.language", &["auto", "zh"], "auto"),
        secret_item("providers.demo"),
        secret_item("providers.other"),
    ]
}

fn secret(value: &str) -> Secret {
    Secret::new(value).expect("收得下")
}

#[test]
fn names_follow_the_name_rules() {
    for good in ["deepseek", "bigmodel-2", "a", "a_b-c9", &"a".repeat(64)] {
        assert!(valid_name(good), "{good}");
    }
    for bad in [
        "",
        "Deepseek",
        "2deepseek",
        "-a",
        "_a",
        "deep seek",
        "深度",
        "a.b",
        &"a".repeat(65),
    ] {
        assert!(!valid_name(bad), "{bad}");
    }
}

#[test]
fn references_are_read_from_toml_json_and_input() {
    let secret_ref = Reference::Secret("deepseek".to_string());
    let env_ref = Reference::Env("DEEPSEEK_API_KEY".to_string());
    assert_eq!(secret_ref.toml(), r#"{ secret = "deepseek" }"#);
    assert_eq!(secret_ref.json(), serde_json::json!({"secret": "deepseek"}));
    assert_eq!(env_ref.name(), "DEEPSEEK_API_KEY");
    assert_eq!(
        Reference::from_input(r#"{ secret = "deepseek" }"#),
        Some(secret_ref.clone())
    );
    assert_eq!(
        Reference::from_input(r#"{env="DEEPSEEK_API_KEY"}"#),
        Some(env_ref.clone())
    );
    assert_eq!(
        Reference::from_json(&serde_json::json!({"env": "DEEPSEEK_API_KEY"})),
        Some(env_ref)
    );
    for bad in [
        r#""deepseek""#,
        r#"{ secret = "Deep Seek" }"#,
        r#"{ secret = 3 }"#,
        r#"{ env = "" }"#,
        r#"{ env = "A=B" }"#,
        r#"{ secret = "a", env = "B" }"#,
        r#"{ key = "a" }"#,
        "{}",
        "deepseek",
    ] {
        assert_eq!(Reference::from_input(bad), None, "{bad}");
    }
    let wanted = Some(Value::Secret(secret_ref.clone()));
    assert_eq!(
        crate::edit::input(Kind::Secret, r#"{ secret = "deepseek" }"#),
        wanted
    );
    assert_eq!(
        crate::edit::from_json(Kind::Secret, &secret_ref.json()),
        wanted
    );
    assert_eq!(
        crate::edit::from_json(Kind::Secret, &serde_json::json!("deepseek")),
        None
    );
    for bad in [
        serde_json::json!("deepseek"),
        serde_json::json!({"secret": "a", "env": "B"}),
        serde_json::json!({"secret": 1}),
        serde_json::json!({}),
    ] {
        assert_eq!(Reference::from_json(&bad), None, "{bad}");
    }
}

#[test]
fn a_secret_never_prints_itself() {
    let key = secret(FAKE);
    assert_eq!(format!("{key:?}"), "Secret(…)");
    assert_eq!(format!("{:?}", Some(&key)), "Some(Secret(…))");
    assert_eq!(key.expose(), FAKE);
}

#[test]
fn set_values_are_trimmed_and_checked() {
    assert_eq!(
        secret(&format!("  {FAKE}\n")).expose(),
        FAKE,
        "粘贴带着换行"
    );
    assert_eq!(Secret::new(" \n\t").map(|_| ()), Err(Refused::Empty));
    assert_eq!(
        Secret::new("sk-FAKE\u{7}x").map(|_| ()),
        Err(Refused::Control)
    );
    assert!(Secret::new(&"x".repeat(VALUE_BYTES)).is_ok(), "正好 16 KiB");
    assert_eq!(
        Secret::new(&"x".repeat(VALUE_BYTES + 1)).map(|_| ()),
        Err(Refused::TooBig)
    );
}

#[test]
fn the_secrets_file_keeps_good_lines_and_reports_bad_ones_without_values() {
    let text = format!(
        "\u{FEFF}# 注释\ndeepseek = \"  {FAKE}  \"\nDeepSeek = \"{FAKE_2}\"\nempty = \"   \"\nnumber = 3\n[table]\nx = \"{FAKE_2}\"\n"
    );
    let stored = parse_file(&text).expect("TOML 写对了");
    assert_eq!(stored.entries.keys().collect::<Vec<_>>(), ["deepseek"]);
    assert_eq!(stored.entries["deepseek"].expose(), FAKE, "去掉前后空白");
    let seen: Vec<(Code, &str, Option<At>)> = stored
        .problems
        .iter()
        .map(|problem| {
            (
                problem.code,
                problem.key.as_deref().unwrap_or_default(),
                problem.at,
            )
        })
        .collect();
    let at = |line, column| Some(At { line, column });
    assert_eq!(
        seen,
        [
            (Code::SecretName, "DeepSeek", at(3, 1)),
            (Code::SecretValue, "empty", at(4, 1)),
            (Code::SecretValue, "number", at(5, 1)),
            (Code::SecretValue, "table", at(6, 2)),
        ]
    );
    for problem in &stored.problems {
        assert_eq!(problem.got, None, "密钥文件里的问题不带收到的原文");
        assert_eq!(problem.severity(), Severity::Error);
    }
    assert_eq!(Code::SecretName.as_str(), "bad_format");
    assert_eq!(Code::SecretValue.as_str(), "wrong_type");
}

#[test]
fn a_broken_secrets_file_never_quotes_the_key() {
    let broken = [
        format!("deepseek = \"{FAKE}\ndeepseek2 = \"{FAKE_2}\"\n"),
        format!("deepseek = {FAKE}\n"),
        format!("deepseek = \"{FAKE}\" \"{FAKE_2}\"\n"),
        format!("deepseek = \"{FAKE}\"\ndeepseek = \"{FAKE_2}\"\n"),
        format!("deepseek = '''{FAKE}\n"),
        format!("{FAKE} {FAKE_2}\n"),
        format!("deepseek = \"{FAKE}\\q\"\n"),
    ];
    for text in broken {
        let problem = parse_file(&text).expect_err("TOML 写错了");
        assert_eq!(problem.code, Code::Syntax);
        let shown = format!("{problem:?}");
        for key in [FAKE, FAKE_2, "FAKE"] {
            assert!(!shown.contains(key), "{text:?} 报的话里有密钥：{shown}");
        }
        let told = tell(&problem, &[], None, &words(&[])).expect("字齐全");
        assert!(!told.message.contains("FAKE"), "{}", told.message);
    }
}

#[test]
fn setting_a_secret_only_touches_its_line() {
    let header = "# Miyu 的密钥：只经 Miyu 写入、替换、删除。\n";
    let first = set_in(header, "deepseek", &secret(FAKE)).expect("放得进");
    assert_eq!(first, format!("{header}deepseek = \"{FAKE}\"\n"));
    let second = set_in(&first, "bigmodel-2", &secret(FAKE_2)).expect("放得进");
    assert_eq!(
        second,
        format!("{first}bigmodel-2 = \"{FAKE_2}\"\n"),
        "接在最后一个后面"
    );
    let replaced = set_in(&second, "deepseek", &secret("sk-FAKE-NEW")).expect("放得进");
    assert_eq!(
        replaced,
        format!("{header}deepseek = \"sk-FAKE-NEW\"\nbigmodel-2 = \"{FAKE_2}\"\n")
    );
    let deleted = unset_in(&replaced, "deepseek").expect("删得掉");
    assert_eq!(deleted, format!("{header}bigmodel-2 = \"{FAKE_2}\"\n"));
    assert_eq!(unset_in(&deleted, "nothing").expect("本来就没有"), deleted);
    let bare = format!("a = \"{FAKE}\"\nb = \"{FAKE_2}\"\n");
    assert_eq!(
        unset_in(&bare, "b").expect("删得掉"),
        format!("a = \"{FAKE}\"\n"),
        "没有注释、只剩一行的也留着"
    );
    assert_eq!(
        unset_in(&format!("a = \"{FAKE}\"\n"), "a").expect("删得掉"),
        ""
    );
    let quoted = set_in("", "odd", &secret("sk-FAKE\"with\\quote")).expect("放得进");
    let stored = parse_file(&quoted).expect("写回去读得懂");
    assert_eq!(stored.entries["odd"].expose(), "sk-FAKE\"with\\quote");
}

#[test]
fn new_lines_go_before_a_table_and_keep_crlf() {
    let text = "# 头\r\n[stray]\r\nx = \"y\"\r\n";
    let set = set_in(text, "deepseek", &secret(FAKE)).expect("放得进");
    assert_eq!(
        set,
        format!("# 头\r\ndeepseek = \"{FAKE}\"\r\n[stray]\r\nx = \"y\"\r\n")
    );
    let stored = parse_file(&set).expect("读得懂");
    assert!(stored.entries.contains_key("deepseek"), "不在那张表里");
    let no_newline = set_in("a = \"b\"", "c", &secret(FAKE)).expect("放得进");
    assert_eq!(no_newline, format!("a = \"b\"\nc = \"{FAKE}\"\n"));
    assert!(
        set_in("[deepseek]\nx = 1\n", "deepseek", &secret(FAKE)).is_err(),
        "写成表的放不进去"
    );
}

#[test]
fn missing_secrets_and_variables_are_warnings() {
    let items = items();
    let text = "[providers]\ndemo = { secret = \"deepseek\" }\nother = { env = \"DEMO_KEY\" }\n";
    let parsed = parse(&items, Layer::Personal, text).expect("写对了");
    assert!(parsed.problems.is_empty(), "{:?}", parsed.problems);
    let nothing = missing(&items, &parsed, Layer::Personal, &|_| false, &|_| false);
    let seen: Vec<(Code, Option<&str>, Option<&str>)> = nothing
        .iter()
        .map(|problem| {
            (
                problem.code,
                problem.key.as_deref(),
                problem.name.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        seen,
        [
            (
                Code::UnknownSecret,
                Some("providers.demo"),
                Some("deepseek")
            ),
            (Code::EnvNotSet, Some("providers.other"), Some("DEMO_KEY")),
        ]
    );
    assert!(
        nothing
            .iter()
            .all(|problem| problem.severity() == Severity::Warning)
    );
    let said: BTreeSet<String> = nothing
        .iter()
        .map(|problem| {
            tell(problem, &items, None, &words(&items))
                .expect("字齐全")
                .message
        })
        .collect();
    assert_eq!(
        said,
        BTreeSet::from([
            "providers.demo 引用的密钥 deepseek 还没设。".to_string(),
            "providers.other 引用的环境变量 DEMO_KEY 核心起来时没有设。".to_string(),
        ])
    );
    let all_set = missing(
        &items,
        &parsed,
        Layer::Personal,
        &|name| name == "deepseek",
        &|name| name == "DEMO_KEY",
    );
    assert!(all_set.is_empty(), "{all_set:?}");
}

#[test]
fn bad_lines_in_the_secrets_file_are_told_plainly() {
    let stored = parse_file("DeepSeek = \"sk-FAKE\"\nempty = \"\"\n").expect("写对了");
    let said: Vec<String> = stored
        .problems
        .iter()
        .map(|problem| {
            tell(problem, &[], None, &words(&[]))
                .expect("字齐全")
                .message
        })
        .collect();
    assert_eq!(
        said,
        [
            "DeepSeek 不能当密钥的名字：小写字母开头，只有小写字母、数字、-、_，最长 64 个字符。",
            "empty 的值要写成带引号的字，不能是空的。",
        ]
    );
}

#[test]
fn a_secret_item_reads_both_forms_and_rejects_the_rest() {
    let items = items();
    let text =
        "[providers]\ndemo = \"sk-FAKE-plain\"\n\n[providers.other]\nsecret = \"deepseek\"\n";
    let parsed = parse(&items, Layer::System, text).expect("写对了");
    assert_eq!(
        parsed.entries["providers.other"].value,
        Value::Secret(Reference::Secret("deepseek".to_string())),
        "有表头的表也认"
    );
    let problem = &parsed.problems[0];
    assert_eq!(problem.code, Code::WrongType, "{problem:?}");
    let told = tell(problem, &items, None, &words(&items)).expect("字齐全");
    assert_eq!(
        told.expected.as_deref(),
        Some(r#"{ secret = "…" } 或 { env = "…" }"#)
    );
    assert_eq!(
        told.message,
        r#"providers.demo 要写 { secret = "…" } 或 { env = "…" }，写的是 "sk-FAKE-plain"。"#
    );
    let resolved = merge(
        &items,
        &Layers {
            system: Some(&parsed),
            ..Layers::default()
        },
        &|_| None,
    );
    assert_eq!(
        resolved
            .get("providers.demo")
            .map(|(value, _)| value.clone()),
        Some(Value::Secret(Reference::Env("EXAMPLE_KEY".to_string()))),
        "写错的照默认值"
    );
}

#[test]
fn a_secret_item_gets_a_one_of_schema() {
    let items = [secret_item("providers.demo")];
    let schema = crate::schema::render(&items, Layer::System, &words(&items)).expect("字齐全");
    let schema: serde_json::Value = serde_json::from_str(&schema).expect("是 JSON");
    let property = &schema["properties"]["providers"]["properties"]["demo"];
    assert_eq!(
        property["default"],
        serde_json::json!({"env": "EXAMPLE_KEY"})
    );
    assert_eq!(
        property["oneOf"][0]["required"],
        serde_json::json!(["secret"])
    );
    assert_eq!(property["oneOf"][1]["properties"]["env"]["type"], "string");
    assert!(crate::list::check(&items).is_empty());
}
