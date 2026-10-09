//! 场所规则里参数的表（`chat.md` 第一条「守着它的」参数那一行、第八条「守着它的」）：行内表、`[rule.chatty]` 表头、点号
//! 连着的键都认；改一项只盖那一项，两条规则改同一项后面的盖，改别的表不影响；来处的行是那一项的键所在的行；表名、项名
//! 拼错给最近的名字；表写成别的、值写错的只丢那一张表、那一项。套上（`Params::at`）：只换改了的几项，别的属性不看，
//! 手造的不合声明的跳过。清理的名单（O-15 下）：两份标记一起写、一样长的照收，长度不同、只写一份、有一份写错的报对的问题、
//! 都不收；`at` 碰到手造的长度不同的照套之前的；按场所改 `invisible` 只改那个场所。

use std::collections::BTreeMap;

use miyu_config::problem::{At, Code};
use miyu_config::{Number, Value};

use super::super::test_support::{factory, group, rules, system, text, value};
use super::super::{Entry, Origin, Resolved, Rules, Source};
use crate::params::test_support::defaults;
use crate::rules::File;
use crate::{Base64, Chatty, Judge, Outbound, Params, Restraint, Window};

/// 一份系统的规则文件 `a.toml`，字是 `body`。
fn file(body: &str) -> [File; 1] {
    [system("a.toml", body)]
}

/// 一组文件（可以有问题）套到群 `1` 上，某一项的值；没有设到的是空的。
fn set(files: &[File], key: &str) -> Option<Value> {
    Rules::parse(files)
        .rules
        .resolve(&group("1"))
        .entries
        .get(key)
        .map(|entry| entry.value.clone())
}

/// 一个小数的值。
fn float(number: f64) -> Value {
    Value::Float(Number::new(number))
}

/// 一条问题看的几样：原因码、键、位置、收到的、最近的名字。
type Seen = (
    Code,
    Option<String>,
    Option<At>,
    Option<String>,
    Option<String>,
);

/// 读一组文件的问题。
fn problems(files: &[File]) -> Vec<Seen> {
    Rules::parse(files)
        .problems
        .into_iter()
        .map(|p| (p.code, p.key, p.at, p.got, p.suggest))
        .collect()
}

/// 一条问题。
fn seen(code: Code, key: &str, line: usize, column: usize, got: &str, near: Option<&str>) -> Seen {
    (
        code,
        Some(key.into()),
        Some(At { line, column }),
        Some(got.into()),
        near.map(String::from),
    )
}

#[test]
fn an_inline_table_sets_only_the_items_it_writes() {
    let files = file("[[rule]]\nchatty = { probability = 80 }\n");
    let resolved = rules(&files).resolve(&group("1"));
    assert_eq!(
        resolved.entries.keys().copied().collect::<Vec<_>>(),
        ["chatty.probability"]
    );
    let entry = &resolved.entries["chatty.probability"];
    assert_eq!(entry.value, Value::Int(80));
    assert_eq!(entry.origin.line, 2);
    assert_eq!(entry.origin.rule, 1);
}

#[test]
fn a_header_table_and_dotted_keys_read_like_an_inline_table() {
    let files = file(
        "[[rule]]\nmatch = { kind = \"group\" }\n[rule.chatty]\nprobability = 80\nbase = 0.9\n\n\
         [[rule]]\njudge.records = 5\n",
    );
    let resolved = rules(&files).resolve(&group("1"));
    let probability = &resolved.entries["chatty.probability"];
    assert_eq!(
        (probability.value.clone(), probability.origin.line),
        (Value::Int(80), 4)
    );
    let base = &resolved.entries["chatty.base"];
    assert_eq!((base.value.clone(), base.origin.line), (float(0.9), 5));
    let records = &resolved.entries["judge.records"];
    assert_eq!(
        (records.value.clone(), records.origin.rule),
        (Value::Int(5), 2)
    );
    assert_eq!(records.origin.line, 8);
}

#[test]
fn later_rules_win_item_by_item_and_other_tables_are_untouched() {
    let files = file(
        "[[rule]]\nchatty = { probability = 80, base = 0.9 }\n\
         [[rule]]\nchatty = { probability = 10 }\njudge = { records = 5 }\n",
    );
    let resolved = rules(&files).resolve(&group("1"));
    let probability = &resolved.entries["chatty.probability"];
    assert_eq!(
        (probability.value.clone(), probability.origin.rule),
        (Value::Int(10), 2)
    );
    let base = &resolved.entries["chatty.base"];
    assert_eq!((base.value.clone(), base.origin.rule), (float(0.9), 1));
    assert_eq!(resolved.entries["judge.records"].value, Value::Int(5));
    assert_eq!(resolved.entries.len(), 3);
}

#[test]
fn durations_and_the_judge_model_stay_as_written() {
    let files = file(
        "[[rule]]\nchatty = { continuation_window = \"20s\" }\njudge = { model = \"@cheap\" }\n",
    );
    let venue = group("1");
    assert_eq!(
        value(&files, &venue, "chatty.continuation_window"),
        Some(text("20s"))
    );
    assert_eq!(value(&files, &venue, "judge.model"), Some(text("@cheap")));
}

#[test]
fn a_misspelt_table_suggests_the_nearest_table_and_the_rest_counts() {
    let files = file("[[rule]]\nchaty = { probability = 80 }\nallow = true\n");
    let unknown = seen(
        Code::UnknownKey,
        "chaty",
        2,
        1,
        "{ probability = 80 }",
        Some("chatty"),
    );
    assert_eq!(problems(&files), [unknown]);
    assert_eq!(set(&files, "allow"), Some(Value::Bool(true)));
    // 属性名拼错照旧给属性名。
    let files = file("[[rule]]\npersna = \"miyu\"\n");
    assert_eq!(problems(&files)[0].4.as_deref(), Some("persona"));
}

#[test]
fn a_misspelt_item_suggests_the_nearest_item_of_that_table() {
    let files = file("[[rule]]\nchatty = { probabilty = 80, base = 0.9 }\n");
    let unknown = seen(
        Code::UnknownKey,
        "chatty.probabilty",
        2,
        12,
        "80",
        Some("chatty.probability"),
    );
    assert_eq!(problems(&files), [unknown]);
    // 别的表里的名字不算：`records` 只在 `judge` 里。
    let files = file("[[rule]]\nchatty = { record = 5 }\n");
    assert_eq!(problems(&files)[0].4, None);
    let files = file("[[rule]]\nchatty = { probabilty = 80, base = 0.9 }\n");
    assert_eq!(set(&files, "chatty.base"), Some(float(0.9)));
}

#[test]
fn a_table_written_as_something_else_drops_only_that_table() {
    let files = file("[[rule]]\nchatty = 5\nallow = true\n");
    assert_eq!(
        problems(&files),
        [seen(Code::WrongType, "chatty", 2, 10, "5", None)]
    );
    assert_eq!(set(&files, "allow"), Some(Value::Bool(true)));
}

#[test]
fn a_bad_item_drops_only_that_item() {
    let files = file(
        "[[rule]]\nchatty = { probability = 2000, base = 0.9 }\n\
         [[rule]]\nchatty = { restraint_k = 0, restraint_cap = nan, continuation_window = \"0s\" }\n",
    );
    assert_eq!(
        problems(&files),
        [
            seen(Code::OutOfRange, "chatty.probability", 2, 26, "2000", None),
            seen(Code::OutOfRange, "chatty.restraint_k", 4, 26, "0", None),
            seen(Code::OutOfRange, "chatty.restraint_cap", 4, 45, "nan", None),
            seen(
                Code::BadFormat,
                "chatty.continuation_window",
                4,
                72,
                "\"0s\"",
                None
            ),
        ]
    );
    let resolved = Rules::parse(&files).rules.resolve(&group("1"));
    assert_eq!(
        resolved.entries.keys().copied().collect::<Vec<_>>(),
        ["chatty.base"]
    );
}

#[test]
fn at_changes_only_what_the_venue_rules_wrote() {
    let files = [factory(
        "50-defaults.toml",
        "[[rule]]\nmatch = { group = [1] }\nchatty = { probability = 80 }\njudge = { model = \"@cheap\" }\n\
         [[rule]]\nchatty = { continuation_window = \"20s\" }\npersona = \"miyu\"\n",
    )];
    let rules = rules(&files);
    let factory = defaults();
    let mut one = factory.clone();
    one.chatty.probability = 80;
    one.chatty.continuation.window = 20_000;
    one.judge.model = Some("@cheap".to_string());
    assert_eq!(factory.at(&rules.resolve(&group("1"))), one);
    let mut two = factory.clone();
    two.chatty.continuation.window = 20_000;
    assert_eq!(factory.at(&rules.resolve(&group("2"))), two);
    // 没有规则设到参数的场所：和出厂的一样。
    assert_eq!(factory.at(&Resolved::default()), factory);
}

#[test]
fn the_judge_persona_is_turned_off_only_where_written() {
    // 某个群想省 token：写一条场所规则关掉，别的群照出厂的开着（施工 O-23 补）。
    let files = [factory(
        "50-defaults.toml",
        "[[rule]]\nmatch = { group = [1] }\njudge = { persona = false }\n",
    )];
    let rules = rules(&files);
    let factory = defaults();
    assert!(factory.judge.persona);
    let one = factory.at(&rules.resolve(&group("1")));
    assert!(!one.judge.persona);
    assert_eq!(
        Params {
            judge: Judge {
                persona: true,
                ..one.judge.clone()
            },
            ..one
        },
        factory,
        "只改了这一项"
    );
    assert!(factory.at(&rules.resolve(&group("2"))).judge.persona);
}

#[test]
fn at_skips_hand_made_entries_that_break_the_declaration() {
    let origin = Origin {
        source: Source::System,
        file: "a.toml".to_string(),
        rule: 1,
        line: 1,
    };
    let entry = |value: Value| Entry {
        value,
        origin: origin.clone(),
    };
    let entries = BTreeMap::from([
        ("chatty.restraint_k", entry(float(0.0))),
        ("chatty.probability", entry(text("80"))),
        ("chatty.half_life", entry(text("3m"))),
        ("persona", entry(text("miyu"))),
        ("chatty.base", entry(float(0.5))),
    ]);
    let factory = defaults();
    let mut expected = factory.clone();
    expected.chatty.base = 0.5;
    assert_eq!(factory.at(&Resolved { entries }), expected);
}

#[test]
fn every_item_lands_in_its_own_field() {
    // 每一项写一个和别的都不一样的数：出厂有几项的数一样（两个 0.25、两个 0.15、两个 0.1、两个 15 秒），放错了格看不出来。
    let files = file(
        "[[rule]]\n\
         [rule.inbound]\nbase64_min_chars = 31\nbase64_max_chars = 4001\nbase64_printable = 801\n\
         [rule.chatty]\nprobability = 41\nbase = 1.1\nrelevance = 1.2\nwillingness = 1.3\nsocial = 1.4\n\
         timing = 1.5\ncontinuity = 1.6\nadjust = 1.7\ndirect = 1.8\ncontinuation = 1.9\n\
         continuation_window = \"21s\"\nafter_speaking = 2.1\nafter_speaking_window = \"22s\"\n\
         restraint = false\nrestraint_half_life = \"23s\"\nrestraint_cap = 2.2\nrestraint_k = 2.3\n\
         severity_min = 9\n\
         [rule.dispatch]\nsupersede_window = \"24s\"\n\
         [rule.judge]\nmodel = \"@cheap\"\nrecords = 25\npersona = false\nmax_tokens = 26\ntimeout = \"27s\"\n\
         moderation_timeout = \"28s\"\nretries = 2\nreason_chars = 29\n\
         [rule.outbound]\nquote_after = 5\nmention_after = \"31s\"\nmin_bigrams = 17\nsimilar = 67\n\
         split_chars = 3001\ninvisible = [\"a\", \"b\"]\nleak_open = [\"<c\"]\nleak_close = [\"c>\"]\n",
    );
    let expected = Params {
        base64: Base64 {
            min_chars: 31,
            max_chars: 4001,
            printable: 801,
        },
        chatty: Chatty {
            probability: 41,
            base: 1.1,
            weights: [1.2, 1.3, 1.4, 1.5, 1.6],
            adjust: 1.7,
            direct: 1.8,
            continuation: Window {
                bonus: 1.9,
                window: 21_000,
            },
            after_speaking: Window {
                bonus: 2.1,
                window: 22_000,
            },
            restraint: Restraint {
                on: false,
                half_life: 23_000,
                cap: 2.2,
                k: 2.3,
            },
            severity_min: 9,
        },
        supersede_window: 24_000,
        judge: Judge {
            model: Some("@cheap".to_string()),
            records: 25,
            persona: false,
            max_tokens: 26,
            timeout: 27_000,
            moderation_timeout: 28_000,
            retries: 2,
            reason_chars: 29,
        },
        outbound: Outbound {
            quote_after: 5,
            mention_after: 31_000,
            min_bigrams: 17,
            similar: 67,
            invisible: vec!['a', 'b'],
            leak_open: vec!["<c".to_string()],
            leak_close: vec!["c>".to_string()],
        },
        split_chars: 3001,
    };
    let resolved = rules(&files).resolve(&group("1"));
    assert_eq!(resolved.entries.len(), crate::params::items::ITEMS.len());
    assert_eq!(defaults().at(&resolved), expected);
}

/// 字的列表的值。
fn texts(items: &[&str]) -> Value {
    Value::List(items.iter().map(|item| text(item)).collect())
}

#[test]
fn markers_written_together_and_as_long_are_taken() {
    let files = file("[[rule]]\noutbound = { leak_open = [\"<a\"], leak_close = [\"a>\"] }\n");
    assert_eq!(set(&files, "outbound.leak_open"), Some(texts(&["<a"])));
    assert_eq!(set(&files, "outbound.leak_close"), Some(texts(&["a>"])));
    let params = defaults().at(&rules(&files).resolve(&group("1")));
    assert_eq!(params.outbound.leak_open, ["<a"]);
    assert_eq!(params.outbound.leak_close, ["a>"]);
}

#[test]
fn markers_of_different_lengths_drop_both_and_blame_the_close() {
    let files = file(
        "[[rule]]\noutbound = { leak_open = [\"<a\"], leak_close = [\"a>\", \"b>\"], similar = 70 }\n",
    );
    let close = seen(
        Code::BadFormat,
        "outbound.leak_close",
        2,
        47,
        "[\"a>\", \"b>\"]",
        None,
    );
    assert_eq!(problems(&files), [close]);
    assert_eq!(set(&files, "outbound.leak_open"), None);
    assert_eq!(set(&files, "outbound.leak_close"), None);
    // 同一张表里别的项照收。
    assert_eq!(set(&files, "outbound.similar"), Some(Value::Int(70)));
}

#[test]
fn a_marker_list_written_alone_is_reported_and_dropped() {
    let files = file("[[rule]]\n[rule.outbound]\nleak_open = [\"<a\"]\nsimilar = 70\n");
    let open = seen(
        Code::BadFormat,
        "outbound.leak_open",
        3,
        13,
        "[\"<a\"]",
        None,
    );
    assert_eq!(problems(&files), [open]);
    assert_eq!(set(&files, "outbound.leak_open"), None);
    assert_eq!(set(&files, "outbound.similar"), Some(Value::Int(70)));
    let files = file("[[rule]]\noutbound.leak_close = [\"a>\"]\n");
    let close = seen(
        Code::BadFormat,
        "outbound.leak_close",
        2,
        23,
        "[\"a>\"]",
        None,
    );
    assert_eq!(problems(&files), [close]);
    assert_eq!(set(&files, "outbound.leak_close"), None);
    // 两条规则各写一份也不算一起写：每一条各报各的。
    let files = file(
        "[[rule]]\noutbound = { leak_open = [\"<a\"] }\n[[rule]]\noutbound = { leak_close = [\"a>\"] }\n",
    );
    assert_eq!(problems(&files).len(), 2);
}

#[test]
fn a_badly_written_marker_list_is_reported_once_and_its_partner_dropped() {
    let files = file("[[rule]]\noutbound = { leak_open = [\"<a\"], leak_close = [\"\"] }\n");
    let close = seen(
        Code::BadFormat,
        "outbound.leak_close",
        2,
        47,
        "[\"\"]",
        None,
    );
    assert_eq!(problems(&files), [close]);
    assert_eq!(set(&files, "outbound.leak_open"), None);
}

#[test]
fn at_keeps_the_markers_it_had_when_hand_made_ones_do_not_pair() {
    let origin = Origin {
        source: Source::System,
        file: "a.toml".to_string(),
        rule: 1,
        line: 1,
    };
    let entry = |value: Value| Entry {
        value,
        origin: origin.clone(),
    };
    let factory = defaults();
    // 出厂两对，只换一份成一个：长度不同，两份照套之前的；别的项照换。
    let entries = BTreeMap::from([
        ("outbound.leak_open", entry(texts(&["<a"]))),
        ("outbound.similar", entry(Value::Int(70))),
    ]);
    let mut expected = factory.clone();
    expected.outbound.similar = 70;
    assert_eq!(factory.at(&Resolved { entries }), expected);
    // 两份都换、长度不同：一样。
    let entries = BTreeMap::from([
        ("outbound.leak_open", entry(texts(&["<a"]))),
        ("outbound.leak_close", entry(texts(&["a>", "b>"]))),
    ]);
    assert_eq!(factory.at(&Resolved { entries }), factory);
    // 只换一份、长度一样：照换。
    let entries = BTreeMap::from([("outbound.leak_open", entry(texts(&["<a", "<b"])))]);
    let mut expected = factory.clone();
    expected.outbound.leak_open = vec!["<a".to_string(), "<b".to_string()];
    assert_eq!(factory.at(&Resolved { entries }), expected);
}

#[test]
fn invisible_changed_for_one_venue_changes_only_that_venue() {
    let files = [factory(
        "50-defaults.toml",
        "[[rule]]\nmatch = { group = [1] }\noutbound = { invisible = [\"~\"] }\n",
    )];
    let rules = rules(&files);
    let factory = defaults();
    assert_eq!(
        factory.at(&rules.resolve(&group("1"))).outbound.invisible,
        ['~']
    );
    assert_eq!(factory.at(&rules.resolve(&group("2"))), factory);
}
