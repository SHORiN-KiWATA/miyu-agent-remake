//! 给模型看的字（施工 8-8 补，`config.md`「类型」）：一行、最多几个字、没有控制字符，CJK 的字占一半以上的不收；从 TOML 和协议
//! 读，Schema 写成什么，期望说要英文，宏怎么声明。

use std::borrow::Cow;

use crate::item::{Item, Kind, Layer};
use crate::problem::Code;
use crate::schema::render;
use crate::test_support::{item, words};
use crate::value::Value;

crate::settings! {
    /// 测试用：一项给模型看的字。
    pub struct Said in "said" {
        /// 给模型看的一句。
        line: Option<String> = none {
            kind: english [12],
            layers: [System, Personal],
            applies: new_session,
            ui: { page: "models", group: "pools", control: text },
        },
    }
}

fn text(text: &str) -> Value {
    Value::Text(Cow::Owned(text.to_string()))
}

#[test]
fn it_is_one_short_line_mostly_not_cjk() {
    let kind = Kind::English { max: 12 };
    for good in ["Quick lookup", "a", "Fast 快速", "x 池", "Twelve chars"] {
        assert_eq!(kind.check(&text(good)), Ok(()), "{good}");
    }
    for bad in [
        "",
        "Thirteen char",
        "two\nlines",
        "tab\there",
        "快速的",
        "池A",
        "ぷーる",
        "풀",
        "，，ab",
        "ＡＢ",
    ] {
        assert_eq!(kind.check(&text(bad)), Err(Code::BadFormat), "{bad}");
    }
    assert_eq!(kind.check(&Value::Bool(true)), Err(Code::WrongType));
    assert_eq!(kind.as_str(), "english");
}

#[test]
fn the_macro_toml_and_the_protocol_read_it() {
    assert_eq!(Said::ITEMS[0].kind, Kind::English { max: 12 });
    let parsed = crate::parse::parse(Said::ITEMS, Layer::System, "[said]\nline = \"Hi there.\"\n")
        .expect("写法对");
    assert!(parsed.problems.is_empty(), "{:?}", parsed.problems);
    assert_eq!(parsed.entries["said.line"].value, text("Hi there."));
    let parsed = crate::parse::parse(
        Said::ITEMS,
        Layer::System,
        "[said]\nline = \"快一点的池\"\n",
    )
    .expect("写法对");
    let codes: Vec<Code> = parsed.problems.iter().map(|problem| problem.code).collect();
    assert_eq!(codes, [Code::BadFormat]);
    let kind = Said::ITEMS[0].kind;
    assert_eq!(
        crate::edit::from_json(kind, &serde_json::json!("Hi.")),
        Some(text("Hi."))
    );
    assert_eq!(crate::edit::input(kind, "\"Hi.\""), Some(text("Hi.")));
    assert!(crate::list::check(Said::ITEMS).is_empty());
    let broken = Item {
        kind: Kind::English { max: 0 },
        default: None,
        ..item("broken.one", &[], "")
    };
    assert_eq!(crate::list::check(&[broken]).len(), 1, "一个字都写不了");
}

#[test]
fn the_schema_and_the_words_say_english() {
    let items: Vec<Item> = Said::ITEMS
        .iter()
        .map(|said| Item {
            key: said.key,
            kind: said.kind,
            default: None,
            ..item(said.key, &[], "")
        })
        .collect();
    let mut said = words(&items);
    said.sentences.insert(
        "config/expected/english".to_string(),
        "最多 {max} 个字的一行英文".to_string(),
    );
    let schema = render(&items, Layer::System, &said).expect("字齐全");
    let json: serde_json::Value = serde_json::from_str(&schema).expect("是 JSON");
    let leaf = &json["properties"]["said"]["properties"]["line"];
    assert_eq!(leaf["type"], "string");
    assert_eq!(leaf["minLength"], 1);
    assert_eq!(leaf["maxLength"], 12);
    let expected = crate::words::expected(&said, Said::ITEMS[0].kind).expect("有字");
    assert_eq!(expected, "最多 12 个字的一行英文");
}
