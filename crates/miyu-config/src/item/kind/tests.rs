//! 施工 8-7 加的三种类型：小数、文字、时长。怎么查、怎么从 TOML 和协议读、Schema 写成什么、说成什么话，宏怎么声明。

use std::borrow::Cow;
use std::time::Duration;

use crate::Number;
use crate::item::{Item, Kind, Layer, duration};
use crate::problem::Code;
use crate::schema::render;
use crate::test_support::{item, words};
use crate::value::{Value, Values};

crate::settings! {
    /// 测试用的三种新类型，和它们的列表。
    pub struct Fresh in "fresh" {
        /// 倍率。
        rate: Option<Number> = none {
            kind: float [0, 1000],
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "models", group: "providers", control: number },
        },
        /// 币种。
        code: Option<String> = none {
            kind: text [3],
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "models", group: "providers", control: text },
        },
        /// 多久一次。
        every: Duration = "24h" {
            kind: duration [60, 2592000],
            layers: [System],
            applies: now,
            ui: { page: "models", group: "catalog", control: text },
        },
        /// 能收的。
        inputs: Option<Vec<String>> = none {
            kind: options ["text", "image"],
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "models", group: "providers", control: list },
        },
        /// 开没开：没写和写了 `false` 分得开。
        local: Option<bool> = none {
            kind: bool,
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "models", group: "providers", control: toggle },
        },
        /// 地址，带默认值。
        url: String = "https://example.invalid/api.json" {
            kind: url,
            layers: [System],
            applies: now,
            ui: { page: "models", group: "catalog", control: text },
        },
    }
}

fn float(number: f64) -> Value {
    Value::Float(Number::new(number))
}

fn text(text: &str) -> Value {
    Value::Text(Cow::Owned(text.to_string()))
}

#[test]
fn a_float_is_in_its_range_and_finite() {
    let kind = Kind::Float { min: 0, max: 10 };
    assert_eq!(kind.check(&float(0.0)), Ok(()));
    assert_eq!(kind.check(&float(10.0)), Ok(()));
    assert_eq!(kind.check(&float(0.5)), Ok(()));
    assert_eq!(kind.check(&float(-0.1)), Err(Code::OutOfRange));
    assert_eq!(kind.check(&float(10.000_1)), Err(Code::OutOfRange));
    assert_eq!(kind.check(&float(f64::NAN)), Err(Code::OutOfRange));
    assert_eq!(kind.check(&float(f64::INFINITY)), Err(Code::OutOfRange));
    assert_eq!(kind.check(&Value::Int(1)), Err(Code::WrongType));
    assert_eq!(kind.as_str(), "float");
}

#[test]
fn a_text_is_short_not_empty_and_has_no_control_characters() {
    let kind = Kind::Text { max: 3 };
    assert_eq!(kind.check(&text("USD")), Ok(()));
    assert_eq!(kind.check(&text("人民币")), Ok(()), "数的是字符，不是字节");
    assert_eq!(kind.check(&text("USDT")), Err(Code::BadFormat));
    assert_eq!(kind.check(&text("")), Err(Code::BadFormat));
    assert_eq!(kind.check(&text("U\nD")), Err(Code::BadFormat));
    assert_eq!(kind.check(&float(1.0)), Err(Code::WrongType));
}

#[test]
fn a_duration_reads_like_the_timeout_flag() {
    assert_eq!(duration("30"), Some(Duration::from_secs(30)));
    assert_eq!(duration("30s"), Some(Duration::from_secs(30)));
    assert_eq!(duration("10m"), Some(Duration::from_secs(600)));
    assert_eq!(duration("24h"), Some(Duration::from_secs(86_400)));
    for bad in [
        "",
        "0",
        "0h",
        "-1h",
        "1.5h",
        "1d",
        "h",
        "1 h",
        "18446744073709551615h",
    ] {
        assert_eq!(duration(bad), None, "{bad:?}");
    }
    let kind = Kind::Duration { min: 60, max: 3600 };
    assert_eq!(kind.check(&text("1m")), Ok(()));
    assert_eq!(kind.check(&text("1h")), Ok(()));
    assert_eq!(kind.check(&text("59s")), Err(Code::OutOfRange));
    assert_eq!(kind.check(&text("61m")), Err(Code::OutOfRange));
    assert_eq!(kind.check(&text("soon")), Err(Code::BadFormat));
}

#[test]
fn the_macro_declares_the_new_kinds_and_reads_them() {
    assert_eq!(Fresh::ITEMS[0].kind, Kind::Float { min: 0, max: 1000 });
    assert_eq!(Fresh::ITEMS[1].kind, Kind::Text { max: 3 });
    assert_eq!(
        Fresh::ITEMS[2].kind,
        Kind::Duration {
            min: 60,
            max: 2_592_000
        }
    );
    assert_eq!(
        Fresh::ITEMS[3].kind,
        Kind::List(&Kind::Option(&["text", "image"]))
    );
    assert!(crate::list::check(Fresh::ITEMS).is_empty());
    // 默认值：时长、网址照写的；没写的几项是空的。
    let fresh = Fresh::from(&Values::defaults(Fresh::ITEMS));
    assert_eq!(fresh.every, Duration::from_secs(86_400));
    assert_eq!(fresh.url, "https://example.invalid/api.json");
    assert_eq!(
        (fresh.rate, fresh.code, fresh.inputs, fresh.local),
        (None, None, None, None)
    );
    // 写了的照写的读。
    let mut values = Values::default();
    values.set("fresh.rate", float(0.5));
    values.set("fresh.every", text("10m"));
    values.set(
        "fresh.inputs",
        Value::List(vec![text("text"), text("image")]),
    );
    values.set("fresh.local", Value::Bool(false));
    let fresh = Fresh::from(&values);
    assert_eq!(fresh.rate.map(Number::get), Some(0.5));
    assert_eq!(fresh.every, Duration::from_secs(600));
    assert_eq!(
        fresh.inputs,
        Some(vec!["text".to_string(), "image".to_string()])
    );
    assert_eq!(fresh.local, Some(false));
}

#[test]
fn toml_and_the_protocol_give_the_new_kinds() {
    let text_of = |source: &str| {
        let parsed = crate::parse::parse(Fresh::ITEMS, Layer::System, source).expect("写法对");
        (parsed.entries, parsed.problems)
    };
    // 小数也收整数；写成字的小数、写成数的时长不收。
    let (entries, problems) =
        text_of("[fresh]\nrate = 1\ncode = \"CNY\"\nevery = \"2h\"\ninputs = [\"image\"]\n");
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(entries["fresh.rate"].value, float(1.0));
    assert_eq!(entries["fresh.every"].value, text("2h"));
    let (_, problems) = text_of("[fresh]\nrate = \"1\"\nevery = 3600\ncode = \"YUAN\"\n");
    let codes: Vec<Code> = problems.iter().map(|problem| problem.code).collect();
    assert_eq!(codes, [Code::WrongType, Code::WrongType, Code::BadFormat]);
    let (_, problems) = text_of("[fresh]\nrate = 1000.5\nevery = \"30s\"\n");
    let codes: Vec<Code> = problems.iter().map(|problem| problem.code).collect();
    assert_eq!(codes, [Code::OutOfRange, Code::OutOfRange]);
    // 协议上的 JSON、人敲的字。
    let rate = Fresh::ITEMS[0].kind;
    assert_eq!(
        crate::edit::from_json(rate, &serde_json::json!(2.5)),
        Some(float(2.5))
    );
    assert_eq!(
        crate::edit::from_json(rate, &serde_json::json!("2.5")),
        None
    );
    assert_eq!(crate::edit::input(rate, "2.5"), Some(float(2.5)));
    assert_eq!(
        crate::edit::input(Fresh::ITEMS[2].kind, "\"1h\""),
        Some(text("1h"))
    );
    // 写回 TOML：整数的小数也带 `.0`，读回来还是小数。
    assert_eq!(float(1.0).toml(), "1.0");
    assert_eq!(float(0.25).toml(), "0.25");
    assert_eq!(float(1.0).json(), serde_json::json!(1.0));
}

#[test]
fn the_schema_and_the_words_say_the_ranges() {
    let items: Vec<Item> = Fresh::ITEMS
        .iter()
        .take(3)
        .map(|fresh| Item {
            key: fresh.key,
            kind: fresh.kind,
            default: fresh.default.clone(),
            ..item(fresh.key, &[], "")
        })
        .collect();
    let mut said = words(&items);
    for (key, sentence) in [
        ("config/expected/float", "{min} 到 {max} 之间的数"),
        ("config/expected/text", "最多 {max} 个字"),
        ("config/expected/duration", "{min} 到 {max}"),
    ] {
        said.sentences.insert(key.to_string(), sentence.to_string());
    }
    let schema = render(&items, Layer::System, &said).expect("字齐全");
    let json: serde_json::Value = serde_json::from_str(&schema).expect("是 JSON");
    let leaf = |name: &str| json["properties"]["fresh"]["properties"][name].clone();
    assert_eq!(leaf("rate")["type"], "number");
    assert_eq!(
        (
            leaf("rate")["minimum"].clone(),
            leaf("rate")["maximum"].clone()
        ),
        (0.into(), 1000.into())
    );
    assert_eq!(leaf("code")["maxLength"], 3);
    assert_eq!(leaf("every")["pattern"], "^[0-9]+[smh]?$");
    assert_eq!(leaf("every")["default"], "24h");
    let rate = crate::words::expected(&said, Fresh::ITEMS[0].kind).expect("有字");
    assert_eq!(rate, "0 到 1000 之间的数");
    let every = crate::words::expected(&said, Fresh::ITEMS[2].kind).expect("有字");
    assert_eq!(every, "1m 到 720h");
}

#[test]
fn a_kind_that_cannot_hold_anything_is_reported() {
    let broken = |kind: Kind| Item {
        kind,
        default: None,
        ..item("broken.one", &[], "")
    };
    for kind in [
        Kind::Float { min: 2, max: 1 },
        Kind::Duration { min: 0, max: 60 },
        Kind::Duration { min: 61, max: 60 },
        Kind::Text { max: 0 },
    ] {
        assert_eq!(crate::list::check(&[broken(kind)]).len(), 1, "{kind:?}");
    }
}
