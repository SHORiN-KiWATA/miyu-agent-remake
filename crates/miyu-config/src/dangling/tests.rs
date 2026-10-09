//! 引用的东西没有（施工 8-8）：引用、模型的列表指的供应商、池没配的报 `bad_reference`，一处一条、带名字；配了的、写在
//! 不能写的那一层的不报；说成话照 `config/no-provider`、`config/no-pool`；模型这一种只收 `<供应商>/<模型>`。

use super::*;
use crate::parse::parse;
use crate::problem::Severity;
use crate::problem::tell;
use crate::test_support::words;
use crate::value::Values;

crate::settings! {
    /// 测试用的用途：一个引用。
    pub struct Uses in "uses" {
        /// 主对话的。
        chat: Option<String> = none {
            kind: reference,
            layers: [System, Personal],
            applies: new_session,
            ui: { page: "models", group: "uses", control: text },
        },
        /// 只能写在系统配置里的一个引用：写在个人设置里不算，也就不查。
        fixed: Option<String> = none {
            kind: reference,
            layers: [System],
            applies: new_session,
            ui: { page: "models", group: "uses", control: text },
        },
    }
}

crate::settings! {
    /// 测试用的池：成员是模型的列表。
    pub struct Pools in "pools.<id>" {
        /// 成员。
        models: Option<Vec<String>> = none {
            kind: models,
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "models", group: "pools", control: list },
        },
    }
}

fn items() -> Vec<Item> {
    [Uses::ITEMS, Pools::ITEMS].concat()
}

/// 当成 `layer` 读 `text`，照「配了的供应商只有 `a`、池只有 `free`」查。
fn found(layer: Layer, text: &str) -> Vec<Problem> {
    let parsed = parse(&items(), layer, text).expect("写法对");
    dangling(&items(), &parsed, layer, &|name| name == "a", &|name| {
        name == "free"
    })
}

/// 每一条的原因码、键、名字。
fn short(problems: &[Problem]) -> Vec<(Code, String, String)> {
    problems
        .iter()
        .map(|problem| {
            (
                problem.code,
                problem.key.clone().unwrap_or_default(),
                problem.name.clone().unwrap_or_default(),
            )
        })
        .collect()
}

#[test]
fn a_reference_to_a_missing_provider_or_pool_is_reported_with_its_name() {
    let problems = found(
        Layer::System,
        "[uses]\nchat = \"b/m\"\nfixed = \"@paid\"\n\n[pools.free]\nmodels = [\"a/x\", \"c/y\", \"a/z\", \"d/w\"]\n",
    );
    assert_eq!(
        short(&problems),
        [
            (
                Code::NoProvider,
                "pools.free.models".to_string(),
                "c".to_string()
            ),
            (
                Code::NoProvider,
                "pools.free.models".to_string(),
                "d".to_string()
            ),
            (Code::NoProvider, "uses.chat".to_string(), "b".to_string()),
            (Code::NoPool, "uses.fixed".to_string(), "paid".to_string()),
        ],
        "照键的先后，列表里一个一条"
    );
    let first = &problems[0];
    assert_eq!(first.code.as_str(), "bad_reference");
    assert_eq!(first.severity(), Severity::Error);
    assert_eq!(
        first.got.as_deref(),
        Some(r#"["a/x", "c/y", "a/z", "d/w"]"#)
    );
    assert_eq!(first.at.map(|at| at.line), Some(6), "指到值那一行");
}

#[test]
fn what_is_there_and_what_does_not_count_is_not_reported() {
    let fine = found(
        Layer::System,
        "[uses]\nchat = \"@free\"\nfixed = \"a/m\"\n\n[pools.free]\nmodels = [\"a/x\"]\n",
    );
    assert_eq!(fine, Vec::new());
    // 写在不能写的那一层：报 `wrong_layer`，不算数，也就不查它指的东西。
    let parsed = parse(&items(), Layer::Personal, "[uses]\nfixed = \"b/m\"\n").expect("写法对");
    assert_eq!(parsed.problems.len(), 1, "{:?}", parsed.problems);
    assert_eq!(
        found(Layer::Personal, "[uses]\nfixed = \"b/m\"\n"),
        Vec::new()
    );
}

#[test]
fn a_dangling_reference_is_said_in_words() {
    let problems = found(Layer::System, "[uses]\nchat = \"b/m\"\nfixed = \"@paid\"\n");
    let words = words(&items());
    let said: Vec<String> = problems
        .iter()
        .map(|problem| {
            tell(problem, &items(), None, &words)
                .expect("字齐全")
                .message
        })
        .collect();
    assert_eq!(
        said,
        [
            "uses.chat 指的供应商 b 没有配。",
            "uses.fixed 指的池 paid 没有配。"
        ]
    );
}

/// 模型这一种（池的成员）：只收 `<供应商>/<模型>`，池、挡位、写法不对的是 `bad_format`；引用照旧两种都收。
#[test]
fn a_model_takes_only_a_provider_and_a_model() {
    let text = |text: &str| Value::Text(text.to_string().into());
    for good in ["a/m", "a/b/c", "dev/DeepSeek V4"] {
        assert_eq!(Kind::Model.check(&text(good)), Ok(()), "{good}");
    }
    for bad in ["@free", "lite", "", "a/", "/m", "A/m"] {
        assert_eq!(Kind::Model.check(&text(bad)), Err(Code::BadFormat), "{bad}");
    }
    assert_eq!(Kind::Reference.check(&text("@free")), Ok(()));
    assert_eq!(Kind::Model.as_str(), "model");
    assert_eq!(Pools::ITEMS[0].kind, Kind::List(&Kind::Model));
    let parsed = parse(
        &items(),
        Layer::System,
        "[pools.free]\nmodels = [\"a/x\", \"@free\"]\n",
    )
    .expect("写法对");
    assert_eq!(
        parsed.problems.iter().map(|p| p.code).collect::<Vec<_>>(),
        [Code::BadFormat],
        "池的成员写了池"
    );
    let mut values = Values::default();
    values.set("pools.free.models", Value::List(vec![text("a/x")]));
    assert_eq!(
        Pools::at(&values, &["free"]).models,
        Some(vec!["a/x".to_string()])
    );
}

crate::settings! {
    /// 测试用的一项 `model_or`（施工 R-5 补）。
    pub struct Embedding in "embedding" {
        /// 算向量的。
        picked: Option<String> = none {
            kind: model_or ["local", "off"],
            layers: [System, Personal],
            applies: next_turn,
            ui: { page: "models", group: "uses", control: text },
        },
    }
}

/// `model_or`（施工 R-5 补）：列出的字不查；写了模型的照 `Model` 查供应商在不在。
#[test]
fn a_model_or_word_is_not_checked_and_its_model_is() {
    let check = |text: &str| {
        let parsed = parse(Embedding::ITEMS, Layer::System, text).expect("写法对");
        dangling(
            Embedding::ITEMS,
            &parsed,
            Layer::System,
            &|name| name == "a",
            &|_| false,
        )
    };
    assert!(check("[embedding]\npicked = \"local\"\n").is_empty());
    assert!(check("[embedding]\npicked = \"a/bge-m3\"\n").is_empty());
    let problems = check("[embedding]\npicked = \"nope/bge-m3\"\n");
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert_eq!(problems[0].code, Code::NoProvider);
}
