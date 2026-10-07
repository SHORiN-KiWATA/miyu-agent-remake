//! 套到一个场所上（`chat.md` 第一条「守着它的」）：后面的规则盖前面的，来处的来源、文件、第几条、第几行都对；没设到的不在
//! 结果里；四种匹配条件各自的对与不对，都写了的要同时满足；整数和字的编号一样；`group` 只配群、`user` 只配私聊；两个都写、
//! 空列表不匹配；不写 `match`、空表匹配所有。仓库里的出厂规则文件读得出、零问题，套到群、私聊上的值对（O-15 下）。

use miyu_config::Value;

use super::super::test_support::{factory, group, private, rules, system, text, value};
use super::{Entry, Origin, Venue};
use crate::rules::{File, Source};

/// 一条规则：设一个 `allow = true`，`match` 那一格写成 `condition`（放在后面：`[rule.match]` 的表头也能写）。
fn allow_when(condition: &str) -> [File; 1] {
    [system(
        "a.toml",
        &format!("[[rule]]\nallow = true\n{condition}\n"),
    )]
}

/// 套到 `venue` 上设到了 `allow`。
fn matches(files: &[File], venue: &Venue) -> bool {
    value(files, venue, "allow").is_some()
}

/// 仓库里的出厂规则文件（`include_str!` 读进来：数据改了，测试跟着变）。
const FACTORY: &str =
    include_str!("../../../../../resources/software/onebot/venues.d/50-defaults.toml");

/// 换到平台 `platform` 上的同一个场所。
fn on(platform: &str, venue: Venue) -> Venue {
    Venue::new(platform, venue.kind(), venue.number()).expect(platform)
}

#[test]
fn later_rules_override_earlier_ones_and_the_origin_follows() {
    let files = [factory(
        "50-defaults.toml",
        "# 出厂\n[[rule]]\nmatch = { kind = \"group\" }\npersona = \"miyu\"\nrate = \"5/300s\"\n\n\
         [[rule]]\nmatch = { group = [123] }\nrate = \"30/60s\"\n",
    )];
    let resolved = rules(&files).resolve(&group("123"));
    let origin = |rule, line| Origin {
        source: Source::Factory,
        file: "50-defaults.toml".to_string(),
        rule,
        line,
    };
    assert_eq!(
        resolved.entries["persona"],
        Entry {
            value: text("miyu"),
            origin: origin(1, 4)
        }
    );
    assert_eq!(
        resolved.entries["rate"],
        Entry {
            value: text("30/60s"),
            origin: origin(2, 9)
        }
    );
    // 别的群只配上第一条。
    let other = rules(&files).resolve(&group("999"));
    assert_eq!(other.entries["rate"].origin, origin(1, 5));
}

#[test]
fn later_files_override_earlier_ones_across_sources() {
    let files = [
        system("10-mine.toml", "[[rule]]\nrate = \"1/1s\"\n"),
        factory("50-defaults.toml", "[[rule]]\nrate = \"5/300s\"\n"),
    ];
    let resolved = rules(&files).resolve(&group("1"));
    assert_eq!(resolved.entries["rate"].origin.source, Source::Factory);
    assert_eq!(resolved.entries["rate"].value, text("5/300s"));
}

#[test]
fn unset_items_are_absent() {
    let files = [system(
        "a.toml",
        "[[rule]]\nmatch = { kind = \"private\" }\nallow = false\n[[rule]]\nmatch = { kind = \"group\" }\nrate = \"0\"\n",
    )];
    let resolved = rules(&files).resolve(&group("1"));
    assert_eq!(
        resolved.entries.keys().copied().collect::<Vec<_>>(),
        ["rate"]
    );
    assert!(rules(&[]).resolve(&group("1")).entries.is_empty());
    let none = [system(
        "a.toml",
        "[[rule]]\nmatch = { platform = \"tg\" }\nallow = true\n",
    )];
    assert!(rules(&none).resolve(&group("1")).entries.is_empty());
}

#[test]
fn every_attribute_keeps_its_value() {
    let files = [system(
        "a.toml",
        "[[rule]]\npersona = \"miyu\"\npreset = \"full\"\ndiscipline = \"every-message\"\nallow = true\n\
         keywords = [\"美羽\", \"miyu\"]\nrate = \"30/1m\"\nparallel = 3\nsleep = \"23:00-07:00\"\n\
         managers = [\"qq:10002\", \"tg:alice\"]\nshow_ids = false\nworkspace = \"/srv/miyu\"\n\
         extra_prompt = \"Be brief.\"\n",
    )];
    let resolved = rules(&files).resolve(&private("1"));
    let got: Vec<(&str, Value)> = resolved
        .entries
        .iter()
        .map(|(key, entry)| (*key, entry.value.clone()))
        .collect();
    let list = |items: &[&str]| Value::List(items.iter().map(|item| text(item)).collect());
    assert_eq!(
        got,
        [
            ("allow", Value::Bool(true)),
            ("discipline", text("every-message")),
            ("extra_prompt", text("Be brief.")),
            ("keywords", list(&["美羽", "miyu"])),
            ("managers", list(&["qq:10002", "tg:alice"])),
            ("parallel", Value::Int(3)),
            ("persona", text("miyu")),
            ("preset", text("full")),
            ("rate", text("30/1m")),
            ("show_ids", Value::Bool(false)),
            ("sleep", text("23:00-07:00")),
            ("workspace", text("/srv/miyu")),
        ]
    );
}

#[test]
fn platform_matches_only_its_platform() {
    let files = allow_when("match = { platform = \"qq\" }");
    assert!(matches(&files, &group("1")));
    assert!(matches(&files, &private("1")));
    assert!(!matches(&files, &on("tg", group("1"))));
    assert!(!matches(&files, &on("qqq", group("1"))));
}

#[test]
fn kind_matches_only_its_kind() {
    let files = allow_when("match = { kind = \"group\" }");
    assert!(matches(&files, &group("1")));
    assert!(!matches(&files, &private("1")));
    let files = allow_when("match = { kind = \"private\" }");
    assert!(!matches(&files, &group("1")));
    assert!(matches(&files, &private("1")));
}

#[test]
fn group_matches_only_listed_groups() {
    let files = allow_when("match = { group = [123456, \"234567\"] }");
    assert!(matches(&files, &group("123456")));
    assert!(matches(&files, &group("234567")));
    assert!(!matches(&files, &group("345678")));
    assert!(!matches(&files, &group("12345")));
    assert!(!matches(&files, &private("123456")));
}

#[test]
fn user_matches_only_listed_private_chats() {
    let files = allow_when("[rule.match]\nuser = [10002]");
    assert!(matches(&files, &private("10002")));
    assert!(!matches(&files, &private("10003")));
    assert!(!matches(&files, &group("10002")));
}

#[test]
fn integer_and_text_ids_compare_as_decimal_text() {
    for written in ["[123]", "[\"123\"]", "[0x7B]", "[1_2_3]", "[+123]"] {
        let files = allow_when(&format!("match = {{ group = {written} }}"));
        assert!(matches(&files, &group("123")), "{written}");
        assert!(!matches(&files, &group("0123")), "{written}");
    }
    // 字原样比，不当成数。
    let files = allow_when("match = { group = [\"0123\"] }");
    assert!(!matches(&files, &group("123")));
    let files = allow_when("match = { user = [-5] }");
    assert!(matches(&files, &private("-5")));
}

#[test]
fn group_and_user_together_match_nothing() {
    let files = allow_when("match = { group = [1], user = [1] }");
    assert!(!matches(&files, &group("1")));
    assert!(!matches(&files, &private("1")));
}

#[test]
fn empty_list_matches_nothing() {
    for condition in ["match = { group = [] }", "match = { user = [] }"] {
        let files = allow_when(condition);
        assert!(!matches(&files, &group("1")), "{condition}");
        assert!(!matches(&files, &private("1")), "{condition}");
    }
}

#[test]
fn no_match_or_an_empty_one_matches_every_venue() {
    for condition in ["", "match = {}", "[rule.match]"] {
        let files = allow_when(condition);
        for venue in [group("1"), private("2"), on("tg", group("3"))] {
            assert!(matches(&files, &venue), "{condition} {venue:?}");
        }
    }
}

#[test]
fn every_written_condition_must_hold() {
    let files = allow_when("match = { platform = \"qq\", kind = \"group\", group = [1] }");
    assert!(matches(&files, &group("1")));
    assert!(!matches(&files, &on("tg", group("1"))));
    assert!(!matches(&files, &group("2")));
    let files = allow_when("match = { kind = \"private\", group = [1] }");
    assert!(!matches(&files, &group("1")));
    assert!(!matches(&files, &private("1")));
}

#[test]
fn the_repository_factory_rules_set_groups_and_private_chats() {
    let files = [factory("50-defaults.toml", FACTORY)];
    let rules = rules(&files);
    let set = |venue: &Venue| {
        let resolved = rules.resolve(venue);
        for entry in resolved.entries.values() {
            assert_eq!(entry.origin.source, Source::Factory);
            assert_eq!(entry.origin.file, "50-defaults.toml");
        }
        resolved
            .entries
            .into_iter()
            .map(|(key, entry)| (key, entry.value))
            .collect::<Vec<_>>()
    };
    let group_set = [
        ("discipline", text("chatty")),
        ("parallel", Value::Int(1)),
        ("rate", text("5/300s")),
    ];
    let private_set = [
        ("discipline", text("every-message")),
        ("parallel", Value::Int(0)),
        ("rate", text("5/300s")),
    ];
    // 不写 `persona`、`preset`、参数：跟着核心的默认和出厂参数走（第一条施工时定的第 16 条）。平台不挑。
    for platform in ["qq", "tg"] {
        assert_eq!(set(&on(platform, group("1"))), group_set);
        assert_eq!(set(&on(platform, private("1"))), private_set);
    }
}
