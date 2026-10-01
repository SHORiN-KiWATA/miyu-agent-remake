//! 键里有人起的名字那一段的项、施工 8-6 加的几种类型（`docs/blueprint/config.md`「类型」，`models.md`「对外的样子」）：
//! 读、合、设置类型、引用取不到的警告、`used_by`、改一项，各走一遍。

use crate::edit::{self, Change};
use crate::merge::{Layers, Origin, merge};
use crate::parse::parse;
use crate::problem::Code;
use crate::secret::Reference;
use crate::{Item, Kind, Layer, Value, Values, key};

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
        /// 地址。
        base_url: Option<String> = none {
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

fn items() -> Vec<Item> {
    [Shop::ITEMS, Size::ITEMS, Uses::ITEMS].concat()
}

fn text(text: &str) -> Value {
    Value::Text(text.to_string().into())
}

/// 当成系统配置读，交回问题的（原因码、键、收到的）。
fn problems(source: &str) -> Vec<(Code, Option<String>, Option<String>)> {
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
fn each_new_kind_reports_its_own_problem() {
    let source = r#"[shops.dev]
driver = "c"
base_url = "ftp://x.invalid"
keys = [{ secret = "Bad" }]
catalog = "Deep Seek"

[shops.dev.models.m]
window = 0

[uses]
chat = "flagship"
"#;
    assert_eq!(
        problems(source),
        [
            (
                Code::NotAnOption,
                Some("shops.dev.driver".to_string()),
                Some(r#""c""#.to_string())
            ),
            (
                Code::BadFormat,
                Some("shops.dev.base_url".to_string()),
                Some(r#""ftp://x.invalid""#.to_string())
            ),
            (
                Code::WrongType,
                Some("shops.dev.keys".to_string()),
                Some(r#"[{ secret = "Bad" }]"#.to_string())
            ),
            (
                Code::BadFormat,
                Some("shops.dev.catalog".to_string()),
                Some(r#""Deep Seek""#.to_string())
            ),
            (
                Code::OutOfRange,
                Some("shops.dev.models.m.window".to_string()),
                Some("0".to_string())
            ),
            (
                Code::BadFormat,
                Some("uses.chat".to_string()),
                Some(r#""flagship""#.to_string())
            ),
        ]
    );
    assert_eq!(
        problems("[shops.dev.models.m]\nwindow = \"8k\"\n")[0].0,
        Code::WrongType
    );
}

#[test]
fn a_reference_is_a_model_or_a_pool() {
    let reference = Kind::Reference;
    for good in ["dev/m", "dev/deepseek/v4", "dev/DeepSeek V4", "@free"] {
        assert!(reference.accepts(&text(good)), "{good}");
    }
    for bad in ["m", "lite", "/m", "dev/", "Dev/m", "@", "@Free", "dev/a\nb"] {
        assert_eq!(reference.check(&text(bad)), Err(Code::BadFormat), "{bad:?}");
    }
    let url = Kind::Url;
    for good in [
        "https://a.invalid",
        "http://relay.invalid:8/v1",
        "HTTPS://A.invalid/x?y",
    ] {
        assert!(url.accepts(&text(good)), "{good}");
    }
    for bad in [
        "a.invalid",
        "https://",
        "https:///v1",
        "https://a b",
        "ftp://a",
    ] {
        assert_eq!(url.check(&text(bad)), Err(Code::BadFormat), "{bad}");
    }
    let int = Kind::Int { min: 1, max: 10 };
    assert!(int.accepts(&Value::Int(1)) && int.accepts(&Value::Int(10)));
    assert_eq!(int.check(&Value::Int(11)), Err(Code::OutOfRange));
    assert_eq!(int.check(&text("1")), Err(Code::WrongType));
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

#[test]
fn a_named_key_is_set_and_unset_with_quotes_where_needed() {
    let window = r#"shops.dev.models."v4.1".window"#;
    let set = edit::apply("", Change::Set(window, &Value::Int(8))).expect("放得进去");
    assert_eq!(set, "[shops.dev.models.\"v4.1\"]\nwindow = 8\n");
    let again = edit::apply(&set, Change::Set(window, &Value::Int(9))).expect("原地换");
    assert_eq!(again, "[shops.dev.models.\"v4.1\"]\nwindow = 9\n");
    let keys = Value::List(vec![Value::Secret(Reference::Secret("a".to_string()))]);
    let both = edit::apply(&again, Change::Set("shops.dev.keys", &keys)).expect("放得进去");
    assert_eq!(
        both,
        "[shops.dev.models.\"v4.1\"]\nwindow = 9\n\n[shops.dev]\nkeys = [{ secret = \"a\" }]\n"
    );
    assert_eq!(
        parse(&items(), Layer::System, &both)
            .expect("读得懂")
            .problems,
        Vec::new()
    );
    let gone = edit::apply(&both, Change::Unset(window)).expect("删得掉");
    assert_eq!(gone, "[shops.dev]\nkeys = [{ secret = \"a\" }]\n");
    assert_eq!(
        edit::input(Kind::List(&Kind::Secret), r#"[{ env = "K" }]"#),
        Some(Value::List(vec![Value::Secret(Reference::Env(
            "K".to_string()
        ))]))
    );
    assert_eq!(
        edit::input(Kind::Int { min: 1, max: 9 }, "3"),
        Some(Value::Int(3))
    );
    assert_eq!(edit::input(Kind::Int { min: 1, max: 9 }, "x"), None);
    assert_eq!(
        edit::from_json(
            Kind::List(&Kind::Secret),
            &serde_json::json!([{"secret": "a"}])
        ),
        Some(keys)
    );
    assert_eq!(
        key::item_of(&items(), window).map(|item| item.key),
        Some("shops.<id>.models.<model>.window")
    );
    assert_eq!(key::item_of(&items(), "shops.Dev.driver"), None);
}

#[test]
fn named_items_are_described_in_the_schema_and_the_reference() {
    let words = crate::test_support::words(&items());
    let schema = crate::schema::render(&items(), Layer::System, &words).expect("字够");
    let schema: serde_json::Value = serde_json::from_str(&schema).expect("是 JSON");
    let shop = &schema["properties"]["shops"]["additionalProperties"]["properties"];
    assert_eq!(shop["base_url"]["format"], "uri");
    assert_eq!(shop["base_url"].get("default"), None, "没有默认值的不写");
    assert_eq!(shop["keys"]["type"], "array");
    assert_eq!(shop["keys"]["default"], serde_json::json!([]));
    assert_eq!(shop["keys"]["items"]["oneOf"][0]["required"][0], "secret");
    let size = &shop["models"]["additionalProperties"]["properties"]["window"];
    assert_eq!(
        (size["minimum"].clone(), size["maximum"].clone()),
        (1.into(), 1000.into())
    );
    assert_eq!(
        size["description"],
        "shops.<id>.models.<model>.window 的说明。能写：1 到 1000 之间的整数。只能写在系统配置或个人设置里。以后开的会话生效。"
    );
    let reference = crate::reference::render(&items(), &words).expect("字够");
    assert!(reference.contains("\n[shops.\"<id>\"]\n"), "{reference}");
    assert!(
        reference.contains(
            "[shops.\"<id>\".models.\"<model>\"]\n# shops.<id>.models.<model>.window 的名字：shops.<id>.models.<model>.window 的说明。\n"
        ),
        "{reference}"
    );
    assert!(
        reference.contains("\n# chat =\n"),
        "没有默认值的写成注释：{reference}"
    );
    assert!(reference.contains("\nkeys = []\n"), "{reference}");
    assert!(
        reference.contains("能写：{ secret = \"…\" } 或 { env = \"…\" } 的列表。"),
        "{reference}"
    );
    assert!(
        toml_edit::Document::parse(reference.as_str()).is_ok(),
        "参考文件照样是读得懂的 TOML"
    );
}

#[test]
fn new_problems_are_told_in_words() {
    let words = crate::test_support::words(&items());
    let told = |source: &str| -> Vec<String> {
        parse(&items(), Layer::System, source)
            .expect("写法对")
            .problems
            .iter()
            .map(|problem| {
                crate::problem::tell(problem, &items(), None, &words)
                    .expect("字够")
                    .message
            })
            .collect()
    };
    assert_eq!(
        told(
            "[shops.Dev]\ndriver = \"a\"\n[shops.dev.models.m]\nwindow = 9999\n[uses]\nchat = \"x\"\n"
        ),
        [
            "shops.Dev 里的 Dev 不能当名字：要写小写字母开头的编号。",
            "shops.dev.models.m.window 要在 1 到 1000 之间，写的是 9999。",
            "uses.chat 要写 <供应商>/<模型> 或 @<池>，写的是 \"x\"。",
        ]
    );
    assert_eq!(
        told("[shops.dev]\ndriver = \"c\"\n"),
        [
            "shops.dev.driver 只能是 a 或 b，写的是 \"c\"。改成其中一个，例如 shops.dev.driver = \"a\"。"
        ],
        "没有默认值的选项照第一个举例"
    );
}
