//! 读规则文件（`chat.md` 第一条「守着它的」）：先后（照文件名、按字节比，系统同名替换出厂，交进来的先后不影响结果，同一
//! 来源重名的后来的盖）；出问题（写法不对整份不用、最上面不认识的键、`rule` 不是表的数组、`match` 写错整条不用、属性
//! 写错只丢一项、不认识的键给最近的名字、BOM、问题的先后、规则从 1 数）。

use miyu_config::Value;
use miyu_config::problem::{At, Code, Severity};

use super::test_support::{factory, group, private, rules, system, text, value};
use super::{File, Problem, Rules, Source};

fn at(line: usize, column: usize) -> Option<At> {
    Some(At { line, column })
}

/// 一条问题看的几样：原因码、文件名、第几条规则、键、位置、收到的、最近的名字。
type Seen = (
    Code,
    String,
    Option<usize>,
    Option<String>,
    Option<At>,
    Option<String>,
    Option<String>,
);

fn problems(files: &[File]) -> Vec<Seen> {
    Rules::parse(files)
        .problems
        .into_iter()
        .map(|p| (p.code, p.file, p.rule, p.key, p.at, p.got, p.suggest))
        .collect()
}

/// 一项的问题：没有最近的名字。
fn item(code: Code, file: &str, rule: usize, key: &str, at: Option<At>, got: &str) -> Seen {
    let file = file.to_string();
    (
        code,
        file,
        Some(rule),
        Some(key.into()),
        at,
        Some(got.into()),
        None,
    )
}

const RATE_5: &str = "[[rule]]\nrate = \"5/300s\"\n";

#[test]
fn files_apply_in_name_order_whatever_order_they_come_in() {
    let a = factory("50-defaults.toml", RATE_5);
    let b = system("80-mine.toml", "[[rule]]\nrate = \"30/60s\"\n");
    let c = system(
        "10-early.toml",
        "[[rule]]\nrate = \"1/1s\"\npersona = \"miyu\"\n",
    );
    let orders = [
        [&a, &b, &c],
        [&a, &c, &b],
        [&b, &a, &c],
        [&b, &c, &a],
        [&c, &a, &b],
        [&c, &b, &a],
    ];
    let first = rules(&[a.clone(), b.clone(), c.clone()]).resolve(&group("1"));
    assert_eq!(first.entries["rate"].value, text("30/60s"));
    assert_eq!(first.entries["rate"].origin.file, "80-mine.toml");
    assert_eq!(first.entries["persona"].origin.file, "10-early.toml");
    for order in orders {
        let files: Vec<File> = order.into_iter().cloned().collect();
        assert_eq!(rules(&files).resolve(&group("1")), first);
    }
}

#[test]
fn names_compare_by_bytes() {
    // 按字节比：`10-` 排在 `9-` 前面，大写排在小写前面，汉字排在 ASCII 后面。
    let files = [
        system("9-a.toml", "[[rule]]\nrate = \"9/1s\"\n"),
        system("10-b.toml", "[[rule]]\nrate = \"10/1s\"\n"),
    ];
    assert_eq!(value(&files, &group("1"), "rate"), Some(text("9/1s")));
    let files = [
        system("b.toml", "[[rule]]\nrate = \"2/1s\"\n"),
        system("B.toml", "[[rule]]\nrate = \"1/1s\"\n"),
        system("群.toml", "[[rule]]\nrate = \"3/1s\"\n"),
    ];
    assert_eq!(value(&files, &group("1"), "rate"), Some(text("3/1s")));
}

#[test]
fn system_file_replaces_factory_file_of_same_name() {
    // 出厂那一份整个不读：它设的 `persona` 不在结果里，它写坏的地方也不报。
    let shipped = factory(
        "50-defaults.toml",
        "[[rule]]\npersona = \"miyu\"\nrate = \"5/300s\"\nbogus = 1\n",
    );
    let mine = system("50-defaults.toml", "[[rule]]\nrate = \"30/60s\"\n");
    for files in [[shipped.clone(), mine.clone()], [mine, shipped]] {
        let resolved = rules(&files).resolve(&group("1"));
        assert_eq!(
            resolved.entries.keys().copied().collect::<Vec<_>>(),
            ["rate"]
        );
        assert_eq!(resolved.entries["rate"].origin.source, Source::System);
        assert_eq!(resolved.entries["rate"].value, text("30/60s"));
    }
    // 不同名的出厂文件照读。
    let files = [
        factory("50-defaults.toml", RATE_5),
        system("80-mine.toml", "[[rule]]\nallow = false\n"),
    ];
    assert_eq!(value(&files, &group("1"), "rate"), Some(text("5/300s")));
}

#[test]
fn later_file_of_same_source_and_name_wins() {
    for make in [factory, system] {
        let files = [
            make("a.toml", "[[rule]]\nrate = \"1/1s\"\nbogus = 1\n"),
            make("a.toml", "[[rule]]\nrate = \"2/1s\"\n"),
        ];
        assert_eq!(value(&files, &group("1"), "rate"), Some(text("2/1s")));
    }
}

#[test]
fn syntax_error_drops_whole_file() {
    let files = [
        system("a.toml", "[[rule]]\nrate = \"0\"\npersona =\n"),
        system("b.toml", "[[rule]]\nallow = true\n"),
    ];
    let read = Rules::parse(&files);
    assert_eq!(read.problems.len(), 1);
    let problem = &read.problems[0];
    assert_eq!(
        (problem.code, problem.file.as_str(), problem.rule),
        (Code::Syntax, "a.toml", None)
    );
    assert_eq!((problem.key.clone(), problem.got.clone()), (None, None));
    assert_eq!(problem.at.map(|at| at.line), Some(3));
    let why = problem.why.clone().unwrap();
    assert!(!why.is_empty() && !why.contains('\n'), "{why}");
    let resolved = read.rules.resolve(&group("1"));
    assert_eq!(
        resolved.entries.keys().copied().collect::<Vec<_>>(),
        ["allow"]
    );
}

#[test]
fn unknown_top_level_key_is_a_warning_and_the_rules_still_count() {
    let files = [system("a.toml", "rules = 1\n[[rule]]\nrate = \"0\"\n")];
    let read = Rules::parse(&files);
    let seen = problems(&files);
    let mut want = item(Code::UnknownKey, "a.toml", 1, "rules", at(1, 1), "1");
    want.2 = None;
    want.6 = Some("rule".into());
    assert_eq!(seen, [want]);
    assert_eq!(read.problems[0].code.severity(), Severity::Warning);
    assert_eq!(
        read.rules.resolve(&group("1")).entries["rate"].value,
        text("0")
    );
}

#[test]
fn rule_must_be_an_array_of_tables() {
    for (written, got, column) in [
        ("rule = 1\n", "1", 8),
        ("rule = [1]\n", "[1]", 8),
        ("rule = [{ rate = \"0\" }, 1]\n", "[{ rate = \"0\" }, 1]", 8),
        ("rule = { rate = \"0\" }\n", "{ rate = \"0\" }", 8),
        ("[rule]\nrate = \"0\"\n", "rule", 2),
    ] {
        let files = [system("a.toml", written)];
        let mut want = item(Code::WrongType, "a.toml", 1, "rule", at(1, column), got);
        want.2 = None;
        assert_eq!(problems(&files), [want], "{written}");
        assert!(
            Rules::parse(&files)
                .rules
                .resolve(&group("1"))
                .entries
                .is_empty()
        );
    }
    // 写在一行里的表的数组也是表的数组；空的数组没有规则，也没有问题。
    let files = [system(
        "a.toml",
        "rule = [{ rate = \"0\" }, { allow = true }]\n",
    )];
    assert_eq!(value(&files, &group("1"), "allow"), Some(Value::Bool(true)));
    assert!(
        rules(&[system("a.toml", "rule = []\n")])
            .resolve(&group("1"))
            .entries
            .is_empty()
    );
}

#[test]
fn bad_match_drops_the_whole_rule() {
    for (written, code, key, got) in [
        ("match = \"all\"", Code::WrongType, "match", "\"all\""),
        (
            "match = { grop = [1] }",
            Code::UnknownKey,
            "match.grop",
            "[1]",
        ),
        (
            "match = { kind = \"groups\" }",
            Code::NotAnOption,
            "match.kind",
            "\"groups\"",
        ),
        ("match = { kind = 1 }", Code::WrongType, "match.kind", "1"),
        (
            "match = { platform = \"QQ\" }",
            Code::BadFormat,
            "match.platform",
            "\"QQ\"",
        ),
        (
            "match = { platform = 1 }",
            Code::WrongType,
            "match.platform",
            "1",
        ),
        (
            "match = { group = 123 }",
            Code::WrongType,
            "match.group",
            "123",
        ),
        (
            "match = { group = [1.5] }",
            Code::WrongType,
            "match.group",
            "[1.5]",
        ),
        (
            "match = { user = [true] }",
            Code::WrongType,
            "match.user",
            "[true]",
        ),
        (
            "match = { group = [\"\"] }",
            Code::BadFormat,
            "match.group",
            "[\"\"]",
        ),
        (
            "match = { user = [\"1 2\"] }",
            Code::BadFormat,
            "match.user",
            "[\"1 2\"]",
        ),
    ] {
        // 第二条规则写对了，照用；第一条的 `allow` 写对了也不用。
        let files = [system(
            "a.toml",
            &format!("[[rule]]\n{written}\nallow = true\n[[rule]]\nrate = \"0\"\n"),
        )];
        let seen = problems(&files);
        assert_eq!(seen.len(), 1, "{written}");
        let (seen_code, _, rule, seen_key, _, seen_got, _) = seen[0].clone();
        assert_eq!(
            (seen_code, rule, seen_key.as_deref(), seen_got.as_deref()),
            (code, Some(1), Some(key), Some(got)),
            "{written}"
        );
        let resolved = Rules::parse(&files).rules.resolve(&private("1"));
        assert_eq!(
            resolved.entries.keys().copied().collect::<Vec<_>>(),
            ["rate"],
            "{written}"
        );
    }
}

#[test]
fn misspelt_match_key_suggests_the_nearest_condition() {
    let files = [system(
        "a.toml",
        "[[rule]]\nmatch = { grop = [1], usr = [2], zzzzz = 1 }\n",
    )];
    let suggests: Vec<_> = problems(&files)
        .into_iter()
        .map(|seen| (seen.3, seen.6))
        .collect();
    assert_eq!(
        suggests,
        [
            (Some("match.grop".into()), Some("match.group".into())),
            (Some("match.usr".into()), Some("match.user".into())),
            (Some("match.zzzzz".into()), None),
        ]
    );
}

#[test]
fn bad_attribute_drops_only_that_item() {
    let files = [system(
        "a.toml",
        "[[rule]]\npersona = 1\ndiscipline = \"loud\"\nparallel = 4\nkeywords = [\"\"]\n\
         allow = true\nrate = 5\nsleep = \"7:00-8:00\"\nmanagers = [\"10002\"]\n[rule.preset]\n",
    )];
    assert_eq!(
        problems(&files),
        [
            item(Code::WrongType, "a.toml", 1, "persona", at(2, 11), "1"),
            item(
                Code::NotAnOption,
                "a.toml",
                1,
                "discipline",
                at(3, 14),
                "\"loud\""
            ),
            item(Code::OutOfRange, "a.toml", 1, "parallel", at(4, 12), "4"),
            item(
                Code::BadFormat,
                "a.toml",
                1,
                "keywords",
                at(5, 12),
                "[\"\"]"
            ),
            item(Code::WrongType, "a.toml", 1, "rate", at(7, 8), "5"),
            item(
                Code::BadFormat,
                "a.toml",
                1,
                "sleep",
                at(8, 9),
                "\"7:00-8:00\""
            ),
            item(
                Code::BadFormat,
                "a.toml",
                1,
                "managers",
                at(9, 12),
                "[\"10002\"]"
            ),
            item(Code::WrongType, "a.toml", 1, "preset", at(10, 7), "preset"),
        ]
    );
    let resolved = Rules::parse(&files).rules.resolve(&group("1"));
    assert_eq!(
        resolved.entries.keys().copied().collect::<Vec<_>>(),
        ["allow"]
    );
}

#[test]
fn unknown_attribute_suggests_the_nearest_name_and_the_rest_count() {
    let files = [system(
        "a.toml",
        "[[rule]]\nrat = \"0\"\nshowids = true\nchatty = { probability = 0.08 }\nallow = true\n",
    )];
    let suggests: Vec<_> = problems(&files)
        .into_iter()
        .map(|seen| (seen.0, seen.3, seen.6))
        .collect();
    let unknown = |key: &str, suggest: Option<&str>| {
        (
            Code::UnknownKey,
            Some(key.to_string()),
            suggest.map(String::from),
        )
    };
    assert_eq!(
        suggests,
        [
            unknown("rat", Some("rate")),
            unknown("showids", Some("show_ids")),
            unknown("chatty", None)
        ]
    );
    let resolved = Rules::parse(&files).rules.resolve(&group("1"));
    assert_eq!(
        resolved.entries.keys().copied().collect::<Vec<_>>(),
        ["allow"]
    );
}

#[test]
fn bom_is_stripped_and_positions_skip_it() {
    let files = [system("a.toml", "\u{feff}[[rule]]\nrate = \"0\"\n")];
    assert_eq!(
        rules(&files).resolve(&group("1")).entries["rate"]
            .origin
            .line,
        2
    );
    let files = [system("a.toml", "\u{feff}rate = 1\n")];
    assert_eq!(problems(&files)[0].4, at(1, 1));
}

#[test]
fn problems_follow_file_order_then_line_and_column() {
    let files = [
        system("b.toml", "[[rule]]\nallow = 1\n"),
        system(
            "a.toml",
            "[[rule]]\nmatch = { kind = 1, platform = 2 }\n[[rule]]\nallow = 1\nx = 1\n",
        ),
    ];
    let seen: Vec<_> = problems(&files)
        .into_iter()
        .map(|seen| (seen.1, seen.2, seen.4))
        .collect();
    let one = |file: &str, rule, line, column| (file.to_string(), Some(rule), at(line, column));
    assert_eq!(
        seen,
        [
            one("a.toml", 1, 2, 18),
            one("a.toml", 1, 2, 32),
            one("a.toml", 2, 4, 9),
            one("a.toml", 2, 5, 1),
            one("b.toml", 1, 2, 9)
        ]
    );
}

#[test]
fn every_problem_names_its_source_and_has_why_only_for_syntax() {
    let files = [
        factory("a.toml", "[[rule]]\nallow = 1\n"),
        system("b.toml", "="),
    ];
    let read = Rules::parse(&files);
    let sources: Vec<_> = read
        .problems
        .iter()
        .map(|p: &Problem| (p.source, p.why.is_some()))
        .collect();
    assert_eq!(sources, [(Source::Factory, false), (Source::System, true)]);
}

#[test]
fn problems_are_sorted_even_when_the_table_order_differs_from_the_lines() {
    // 点号连着写的 `match.…` 在表里排在一起，问题照行排：`match.kind`、`allow`、`match.platform`。
    let files = [system(
        "a.toml",
        "[[rule]]\nmatch.kind = 1\nallow = 1\nmatch.platform = 2\n",
    )];
    let seen: Vec<_> = problems(&files)
        .into_iter()
        .map(|seen| (seen.3, seen.4))
        .collect();
    let one = |key: &str, line, column| (Some(key.to_string()), at(line, column));
    assert_eq!(
        seen,
        [
            one("match.kind", 2, 14),
            one("allow", 3, 9),
            one("match.platform", 4, 18)
        ]
    );
}
