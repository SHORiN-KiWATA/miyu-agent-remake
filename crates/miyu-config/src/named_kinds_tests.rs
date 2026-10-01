//! 施工 8-6 加的几种类型（网址、名字、引用、整数、列表）怎么查、人敲的字和协议上的 JSON 怎么读、改一项、生成 Schema
//! 和参考文件、报错说成话，各走一遍。施工 8-6b：网址也能是环境变量的引用（`{ env = … }`），`check`、`edit::input`、
//! `edit::from_json`、Schema、参考文件都接上。清单、`Shop`、`items`/`text`/`problems` 共用 `named_tests.rs` 的。

use crate::edit::{self, Change};
use crate::merge::{Layers, merge};
use crate::named_tests::{Shop, items, problems, text};
use crate::parse::parse;
use crate::problem::Code;
use crate::secret::Reference;
use crate::{Address, Kind, Layer, Value, key};

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
    // 网址也能是环境变量的引用（施工 8-6b），但不能是密钥的引用：地址不进密钥文件。
    let env_ref = Value::Secret(Reference::Env("RELAY_URL".to_string()));
    assert_eq!(url.check(&env_ref), Ok(()));
    let secret_ref = Value::Secret(Reference::Secret("relay".to_string()));
    assert_eq!(url.check(&secret_ref), Err(Code::WrongType));
    let int = Kind::Int { min: 1, max: 10 };
    assert!(int.accepts(&Value::Int(1)) && int.accepts(&Value::Int(10)));
    assert_eq!(int.check(&Value::Int(11)), Err(Code::OutOfRange));
    assert_eq!(int.check(&text("1")), Err(Code::WrongType));
}

/// 网址也能是环境变量的引用（施工 8-6b）：写死的照字读，`{ env = … }` 照引用读；`{ secret = … }` 读得出引用的字节
/// （和密钥同一种读法），但不是网址的合法类型，`check` 挡在 `wrong_type`。设置类型读出 [`Address`]，不解出地址本身。
#[test]
fn a_url_item_takes_a_literal_address_or_an_env_reference() {
    assert_eq!(
        edit::input(Kind::Url, r#"{ env = "RELAY_URL" }"#),
        Some(Value::Secret(Reference::Env("RELAY_URL".to_string())))
    );
    assert_eq!(
        edit::input(Kind::Url, r#""https://a.invalid""#),
        Some(text("https://a.invalid"))
    );
    assert_eq!(
        edit::input(Kind::Url, r#"{ secret = "relay" }"#),
        Some(Value::Secret(Reference::Secret("relay".to_string()))),
        "读得出字节，check 才挡"
    );
    assert_eq!(
        edit::from_json(Kind::Url, &serde_json::json!({"env": "RELAY_URL"})),
        Some(Value::Secret(Reference::Env("RELAY_URL".to_string())))
    );
    assert_eq!(
        edit::from_json(Kind::Url, &serde_json::json!("https://a.invalid")),
        Some(text("https://a.invalid"))
    );
    let source = "[shops.dev]\nbase_url = { env = \"RELAY_URL\" }\n\n[shops.plain]\nbase_url = \"https://a.invalid\"\n";
    let parsed = parse(&items(), Layer::System, source).expect("写法对");
    assert!(parsed.problems.is_empty(), "{:?}", parsed.problems);
    assert_eq!(
        parsed.entries["shops.dev.base_url"].value,
        Value::Secret(Reference::Env("RELAY_URL".to_string()))
    );
    let layers = Layers {
        system: Some(&parsed),
        ..Layers::default()
    };
    let values = merge(&items(), &layers, &|_| None).values();
    assert_eq!(
        Shop::at(&values, &["dev"]).base_url,
        Some(Address::Env("RELAY_URL".to_string()))
    );
    assert_eq!(
        Shop::at(&values, &["plain"]).base_url,
        Some(Address::Literal("https://a.invalid".to_string()))
    );
    // 密钥的引用不是网址合法的写法：和别的乱写法一样落到 `wrong_type`，不进最终值。
    let bad = problems("[shops.dev]\nbase_url = { secret = \"relay\" }\n");
    assert_eq!(
        bad,
        [(
            Code::WrongType,
            Some("shops.dev.base_url".to_string()),
            Some(r#"{ secret = "relay" }"#.to_string())
        )]
    );
    // 没设的环境变量一样算「没设」：取不到的查法照第九条，`missing` 不分密钥、网址。
    let missing = crate::secret::missing(&items(), &parsed, Layer::System, &|_| false, &|_| false);
    let seen: Vec<(Code, Option<&str>, Option<&str>)> = missing
        .iter()
        .map(|p| (p.code, p.key.as_deref(), p.name.as_deref()))
        .collect();
    assert_eq!(
        seen,
        [(
            Code::EnvNotSet,
            Some("shops.dev.base_url"),
            Some("RELAY_URL")
        )]
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
    // 写死的地址，或者一个环境变量的引用（施工 8-6b，没有 `{ secret = … }`：地址不进密钥文件）。
    assert_eq!(
        shop["base_url"]["oneOf"],
        serde_json::json!([
            {"type": "string", "format": "uri"},
            {
                "additionalProperties": false,
                "properties": {"env": {"type": "string"}},
                "required": ["env"],
                "type": "object",
            },
        ])
    );
    assert_eq!(
        shop["base_url"]["description"],
        "shops.<id>.base_url 的说明。能写：http:// 或 https:// 开头的网址 或 { env = \"…\" }。只能写在系统配置或个人设置里。下一个回合开始时生效。"
    );
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
        reference.contains("能写：http:// 或 https:// 开头的网址 或 { env = \"…\" }。"),
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
