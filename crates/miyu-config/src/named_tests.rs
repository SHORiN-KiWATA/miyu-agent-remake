//! 键里有人起的名字那一段的项（`docs/blueprint/config.md`「类型」，`models.md`「对外的样子」，施工 8-6）：真的键怎么拆、
//! 怎么接，合并时只合写了的真的键，引用取不到的警告、`used_by`。类型怎么查、改一项、生成 Schema 和参考文件、说成话在
//! `named_kinds_tests.rs`（施工 8-6b：网址也能是环境变量的引用）；清单、`Shop`、`items`/`text`/`problems` 两份共用这里的。

use crate::merge::{Layers, Origin, merge};
use crate::parse::parse;
use crate::problem::Code;
use crate::secret::Reference;
use crate::{Address, Item, Layer, Value, Values, key};

crate::settings! {
    /// 测试用的一家：没有默认值的选项、网址、名字，空的密钥列表。
    pub struct Shop in "shops.<id>" {
        /// 驱动。
        driver: Option<String> = none {
            kind: option ["a", "b"],
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "models", group: "providers", control: select },
        },
        /// 地址，可能是写死的，也可能是一个环境变量的引用（施工 8-6b）。
        base_url: Option<Address> = none {
            kind: url,
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "models", group: "providers", control: text },
        },
        /// 几个 key。
        keys: Vec<Reference> = [] {
            kind: secrets,
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "models", group: "providers", control: list },
        },
        /// 对应的一家。
        catalog: Option<String> = none {
            kind: name,
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "models", group: "providers", control: text },
        },
    }
}

crate::settings! {
    /// 测试用的一个模型的资料。
    pub struct Size in "shops.<id>.models.<model>" {
        /// 窗口。
        window: Option<i64> = none {
            kind: int [1, 1000],
            layers: [System, Personal],
            applies: new_session,
            ui: { page: "models", group: "providers", control: number },
        },
    }
}

crate::settings! {
    /// 测试用的用途。
    pub struct Uses in "uses" {
        /// 主对话。
        chat: Option<String> = none {
            kind: reference,
            layers: [System, Personal],
            applies: new_session,
            ui: { page: "models", group: "uses", control: text },
        },
    }
}

/// `named_kinds_tests.rs` 也用这份清单。
pub(crate) fn items() -> Vec<Item> {
    [Shop::ITEMS, Size::ITEMS, Uses::ITEMS].concat()
}

/// `named_kinds_tests.rs` 也用它造字。
pub(crate) fn text(text: &str) -> Value {
    Value::Text(text.to_string().into())
}

/// 当成系统配置读，交回问题的（原因码、键、收到的）。`named_kinds_tests.rs` 也用它。
pub(crate) fn problems(source: &str) -> Vec<(Code, Option<String>, Option<String>)> {
    parse(&items(), Layer::System, source)
        .expect("写法对")
        .problems
        .into_iter()
        .map(|problem| (problem.code, problem.key, problem.got))
        .collect()
}

const GOOD: &str = r#"[shops.dev]
driver = "a"
base_url = "https://relay.example.invalid/v1"
keys = [{ secret = "dev" }, { env = "DEV_KEY" }]
catalog = "deepseek"

[shops.dev.models."v4.1-flash"]
window = 1000

[uses]
chat = "dev/v4.1-flash"
"#;

#[test]
fn named_items_are_read_under_their_real_keys() {
    let parsed = parse(&items(), Layer::System, GOOD).expect("写法对");
    assert!(parsed.problems.is_empty(), "{:?}", parsed.problems);
    let keys: Vec<&str> = parsed.entries.keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        [
            "shops.dev.base_url",
            "shops.dev.catalog",
            "shops.dev.driver",
            "shops.dev.keys",
            r#"shops.dev.models."v4.1-flash".window"#,
            "uses.chat",
        ]
    );
    let entry = &parsed.entries[r#"shops.dev.models."v4.1-flash".window"#];
    assert_eq!(
        (entry.item, &entry.value, entry.line),
        ("shops.<id>.models.<model>.window", &Value::Int(1000), 8)
    );
    assert_eq!(
        parsed.entries["shops.dev.keys"].value,
        Value::List(vec![
            Value::Secret(Reference::Secret("dev".to_string())),
            Value::Secret(Reference::Env("DEV_KEY".to_string())),
        ])
    );
}

#[test]
fn a_badly_named_table_is_one_problem_and_nothing_under_it_counts() {
    let source = "[shops.Dev]\ndriver = \"a\"\nbase_url = \"x\"\n\n[shops.ok]\ndriver = \"b\"\n";
    let parsed = parse(&items(), Layer::System, source).expect("写法对");
    assert_eq!(parsed.problems.len(), 1, "{:?}", parsed.problems);
    let problem = &parsed.problems[0];
    assert_eq!(
        (
            problem.code,
            problem.key.as_deref(),
            problem.name.as_deref(),
            problem.why.as_deref(),
            problem.got.as_deref()
        ),
        (
            Code::BadSegment,
            Some("shops.Dev"),
            Some("Dev"),
            Some(key::ID),
            None
        )
    );
    assert_eq!(problem.code.as_str(), "bad_format");
    let keys: Vec<&str> = parsed.entries.keys().map(String::as_str).collect();
    assert_eq!(keys, ["shops.ok.driver"], "写法对的那一家照收");
    assert_eq!(
        problems("shops.Dev.driver = \"a\"\n")[0].0,
        Code::BadSegment,
        "点号连着写的也一样"
    );
}

#[test]
fn layers_merge_per_real_key_and_no_default_means_absent() {
    let system = parse(&items(), Layer::System, GOOD).expect("写法对");
    let personal = parse(
        &items(),
        Layer::Personal,
        "[shops.dev]\nbase_url = \"https://mine.invalid\"\n[shops.other]\ndriver = \"b\"\n",
    )
    .expect("写法对");
    let layers = Layers {
        system: Some(&system),
        personal: Some(&personal),
        project: None,
    };
    let resolved = merge(&items(), &layers, &|_| None);
    assert_eq!(
        resolved.get("shops.dev.base_url"),
        Some((
            &text("https://mine.invalid"),
            &Origin::File {
                layer: Layer::Personal,
                line: 2
            }
        ))
    );
    assert_eq!(
        resolved.get("shops.other.base_url"),
        None,
        "没写、没有默认值"
    );
    assert_eq!(resolved.get("shops.other.keys"), None, "只合写了的真的键");
    let values = resolved.values();
    assert_eq!(
        key::names(values.keys(), "shops.<id>", &[]),
        ["dev", "other"]
    );
    let dev = Shop::at(&values, &["dev"]);
    assert_eq!(dev.driver.as_deref(), Some("a"));
    assert_eq!(dev.keys.len(), 2);
    let other = Shop::at(&values, &["other"]);
    assert_eq!(
        (other.base_url, other.keys),
        (None, Vec::new()),
        "没写的照默认值"
    );
    assert_eq!(Size::at(&values, &["dev", "v4.1-flash"]).window, Some(1000));
    assert_eq!(Size::at(&values, &["dev", "nope"]).window, None);
    assert_eq!(Uses::from(&values).chat.as_deref(), Some("dev/v4.1-flash"));
    assert_eq!(Uses::from(&Values::default()).chat, None);
    assert_eq!(
        Values::defaults(&items()).keys().count(),
        0,
        "人起的名字的项、没有默认值的项不在默认值里"
    );
}

#[test]
fn references_in_a_list_are_each_checked_and_listed_as_used() {
    let parsed = parse(&items(), Layer::System, GOOD).expect("写法对");
    let missing = crate::secret::missing(&items(), &parsed, Layer::System, &|_| false, &|_| false);
    let seen: Vec<(Code, Option<&str>, Option<&str>)> = missing
        .iter()
        .map(|p| (p.code, p.key.as_deref(), p.name.as_deref()))
        .collect();
    assert_eq!(
        seen,
        [
            (Code::UnknownSecret, Some("shops.dev.keys"), Some("dev")),
            (Code::EnvNotSet, Some("shops.dev.keys"), Some("DEV_KEY")),
        ]
    );
    let layers = Layers {
        system: Some(&parsed),
        ..Layers::default()
    };
    let values = merge(&items(), &layers, &|_| None).values();
    assert_eq!(
        crate::secret::used(&values),
        [("dev".to_string(), "shops.dev.keys".to_string())]
    );
}
