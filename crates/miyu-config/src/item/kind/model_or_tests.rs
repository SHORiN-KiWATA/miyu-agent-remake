//! 施工 R-5 补加的类型 `model_or`：列出的几个字之一，或者一个模型 `<供应商>/<模型>`（不收池：向量要和存下的同一个模型比，
//! 池里换成员就对不上）。怎么查、宏怎么声明、从 TOML 和协议读、Schema 写成什么、说成什么话。跨项查供应商在 `dangling` 的测试里。

use std::borrow::Cow;

use crate::item::{Item, Kind, Layer};
use crate::problem::Code;
use crate::schema::render;
use crate::test_support::{item, words};
use crate::value::{Value, Values};

crate::settings! {
    /// 测试用的一项 `model_or`。
    pub struct Picked in "picked" {
        /// 算向量的模型。
        embedding: Option<String> = none {
            kind: model_or ["local", "off"],
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "models", group: "uses", control: text },
        },
    }
}

const KIND: Kind = Kind::ModelOr(&["local", "off"]);

fn text(text: &str) -> Value {
    Value::Text(Cow::Owned(text.to_string()))
}

#[test]
fn a_listed_word_or_a_model_is_accepted() {
    for good in [
        "local",
        "off",
        "siliconflow/BAAI/bge-m3",
        "dev/text-embedding-3-small",
    ] {
        assert_eq!(KIND.check(&text(good)), Ok(()), "{good}");
    }
}

#[test]
fn a_pool_and_anything_else_are_refused() {
    for (bad, code) in [
        ("@cheap", Code::BadFormat),
        ("Local", Code::BadFormat),
        ("nope", Code::BadFormat),
        ("", Code::BadFormat),
        ("/model", Code::BadFormat),
    ] {
        assert_eq!(KIND.check(&text(bad)), Err(code), "{bad:?}");
    }
    assert_eq!(KIND.check(&Value::Bool(true)), Err(Code::WrongType));
    assert_eq!(KIND.from_env("local"), None, "环境变量不压过它");
}

#[test]
fn it_is_named_model_or() {
    assert_eq!(KIND.as_str(), "model_or");
}

#[test]
fn the_macro_declares_it_and_reads_it() {
    assert_eq!(Picked::ITEMS[0].kind, KIND);
    assert!(crate::list::check(Picked::ITEMS).is_empty());
    assert_eq!(
        Picked::from(&Values::defaults(Picked::ITEMS)).embedding,
        None
    );
    let parsed = crate::parse::parse(
        Picked::ITEMS,
        Layer::System,
        "[picked]\nembedding = \"dev/bge-m3\"\n",
    )
    .expect("写法对");
    assert!(parsed.problems.is_empty(), "{:?}", parsed.problems);
    assert_eq!(parsed.entries["picked.embedding"].value, text("dev/bge-m3"));
    let parsed = crate::parse::parse(
        Picked::ITEMS,
        Layer::System,
        "[picked]\nembedding = \"@pool\"\n",
    )
    .expect("写法对");
    let codes: Vec<Code> = parsed.problems.iter().map(|problem| problem.code).collect();
    assert_eq!(codes, [Code::BadFormat]);
    assert_eq!(
        crate::edit::from_json(KIND, &serde_json::json!("off")),
        Some(text("off"))
    );
    assert_eq!(
        crate::edit::input(KIND, "dev/bge-m3"),
        Some(text("dev/bge-m3"))
    );
}

#[test]
fn the_schema_and_the_words_say_the_words_or_a_model() {
    let items: Vec<Item> = vec![Item {
        key: Picked::ITEMS[0].key,
        kind: KIND,
        default: None,
        ..item(Picked::ITEMS[0].key, &[], "")
    }];
    let said = words(&items);
    let schema = render(&items, Layer::System, &said).expect("字齐全");
    let json: serde_json::Value = serde_json::from_str(&schema).expect("是 JSON");
    let leaf = json["properties"]["picked"]["properties"]["embedding"].clone();
    let any = leaf["anyOf"]
        .as_array()
        .unwrap_or_else(|| panic!("有 anyOf：{leaf}"));
    assert_eq!(any[0]["enum"], serde_json::json!(["local", "off"]));
    assert_eq!(any[1]["type"], "string");
    assert!(
        any[1]["pattern"]
            .as_str()
            .is_some_and(|pattern| pattern.contains('/')),
        "{leaf}"
    );
    let expected = crate::words::expected(&said, KIND).expect("有字");
    let model = crate::words::expected(&said, Kind::Model).expect("有字");
    assert!(expected.starts_with("local"), "{expected}");
    assert!(
        expected.contains("off") && expected.ends_with(&model),
        "{expected}"
    );
}

#[test]
fn a_list_of_words_that_cannot_work_is_reported() {
    let listed = |kind: Kind| Item {
        kind,
        default: None,
        ..item("broken.one", &[], "")
    };
    for (kind, problem) in [
        (Kind::ModelOr(&[]), "至少列一个字"),
        (Kind::ModelOr(&["local", "local"]), "字写重了"),
        (Kind::ModelOr(&["local", "a/b"]), "不能带 /"),
    ] {
        let problems = crate::list::check(&[listed(kind)]);
        assert_eq!(problems.len(), 1, "{kind:?}：{problems:?}");
        assert!(problems[0].contains(problem), "{problems:?}");
    }
    assert!(crate::list::check(&[listed(Kind::ModelOr(&["local"]))]).is_empty());
}
