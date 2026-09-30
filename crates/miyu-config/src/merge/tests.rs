//! 分层：四层的先后和来源；环境变量压过、读不懂的当没设；一项写错只丢这一项，照下面几层或默认值；项目配置收紧的收、
//! 宽的不算、一样的收、不能写的不算、没信任的不算；每一层写的列得出来。

use std::borrow::Cow;
use std::collections::BTreeMap;

use crate::item::{Item, Kind, Layer, Tighten};
use crate::merge::{Layers, Origin, Trust, Written, below, explain, merge};
use crate::parse::{Parsed, parse};
use crate::problem::Code;
use crate::test_support::{item, system_only};
use crate::value::Value;

fn items() -> Vec<Item> {
    vec![
        item("ui.language", &["auto", "zh", "en"], "auto"),
        Item {
            env: Some("MIYU_LOG"),
            ..system_only(item("log.level", &["info", "debug"], "info"))
        },
        Item {
            kind: Kind::Bool,
            default: Value::Bool(false),
            layers: &[Layer::System, Layer::Personal, Layer::Project],
            tighten: Some(Tighten::TrueOnly),
            ..item("permission.start_read_only", &[], "")
        },
    ]
}

fn text(text: &'static str) -> Value {
    Value::Text(Cow::Borrowed(text))
}

fn read(layer: Layer, source: &str) -> Parsed {
    parse(&items(), layer, source).expect("写法对")
}

fn line(layer: Layer, line: usize) -> Origin {
    Origin::File { layer, line }
}

/// 没有环境变量。
fn no_env(_: &str) -> Option<String> {
    None
}

#[test]
fn the_upper_layer_wins_and_says_where_it_came_from() {
    let system = read(
        Layer::System,
        "[ui]\nlanguage = \"en\"\n[log]\nlevel = \"debug\"\n",
    );
    let personal = read(Layer::Personal, "\n\n[ui]\nlanguage = \"zh\"\n");
    let layers = Layers {
        system: Some(&system),
        personal: Some(&personal),
        project: None,
    };
    let resolved = merge(&items(), &layers, &no_env);
    assert_eq!(
        resolved.get("ui.language"),
        Some((&text("zh"), &line(Layer::Personal, 4)))
    );
    assert_eq!(
        resolved.get("log.level"),
        Some((&text("debug"), &line(Layer::System, 4)))
    );
    assert_eq!(
        resolved.get("permission.start_read_only"),
        Some((&Value::Bool(false), &Origin::Default))
    );
    assert_eq!(resolved.get("nothing.here"), None);
    assert_eq!(resolved.values().get("ui.language"), Some(&text("zh")));
    assert!(resolved.problems.is_empty());
}

#[test]
fn nothing_written_is_every_default() {
    let resolved = merge(&items(), &Layers::default(), &no_env);
    for item in items() {
        assert_eq!(
            resolved.get(item.key),
            Some((&item.default, &Origin::Default))
        );
    }
}

#[test]
fn an_environment_variable_wins_when_understood() {
    let system = read(Layer::System, "log.level = \"info\"\n");
    let layers = Layers {
        system: Some(&system),
        ..Layers::default()
    };
    let env = |name: &str| (name == "MIYU_LOG").then(|| "DEBUG".to_string());
    let resolved = merge(&items(), &layers, &env);
    assert_eq!(
        resolved.get("log.level"),
        Some((&text("debug"), &Origin::Env("MIYU_LOG")))
    );
    assert!(resolved.ignored_env.is_empty());
    let loud = |_: &str| Some("loud".to_string());
    let resolved = merge(&items(), &layers, &loud);
    assert_eq!(
        resolved.get("log.level"),
        Some((&text("info"), &line(Layer::System, 1))),
        "读不懂的当没设，照配置"
    );
    assert_eq!(resolved.ignored_env, [("MIYU_LOG", "loud".to_string())]);
    let blank = |_: &str| Some("  ".to_string());
    let resolved = merge(&items(), &layers, &blank);
    assert!(resolved.ignored_env.is_empty(), "空的当没设，不报");
    assert_eq!(
        resolved.get("log.level").map(|(_, origin)| origin),
        Some(&line(Layer::System, 1))
    );
}

#[test]
fn a_broken_item_falls_back_to_the_layers_below_and_the_rest_stays() {
    let system = read(Layer::System, "ui.language = \"en\"\n");
    let personal = read(
        Layer::Personal,
        "ui.language = \"cn\"\npermission.start_read_only = true\n",
    );
    assert_eq!(personal.problems.len(), 1);
    let layers = Layers {
        system: Some(&system),
        personal: Some(&personal),
        project: None,
    };
    let resolved = merge(&items(), &layers, &no_env);
    assert_eq!(
        resolved.get("ui.language"),
        Some((&text("en"), &line(Layer::System, 1)))
    );
    assert_eq!(
        resolved.get("permission.start_read_only"),
        Some((&Value::Bool(true), &line(Layer::Personal, 2))),
        "别的照常"
    );
    let item = &items()[0];
    assert_eq!(
        below(item, &layers, Layer::Personal),
        (text("en"), line(Layer::System, 1))
    );
    assert_eq!(
        below(item, &layers, Layer::System),
        (text("auto"), Origin::Default)
    );
}

#[test]
fn a_trusted_project_only_tightens() {
    let on = read(Layer::Project, "[permission]\nstart_read_only = true\n");
    let off = read(Layer::Project, "[permission]\nstart_read_only = false\n");
    let system_on = read(Layer::System, "permission.start_read_only = true\n");
    let key = "permission.start_read_only";

    let resolved = merge(
        &items(),
        &Layers {
            project: Some((&on, Trust::Trusted)),
            ..Layers::default()
        },
        &no_env,
    );
    assert_eq!(
        resolved.get(key),
        Some((&Value::Bool(true), &line(Layer::Project, 2))),
        "收紧的收"
    );
    assert!(resolved.problems.is_empty());

    let layers = Layers {
        system: Some(&system_on),
        project: Some((&off, Trust::Trusted)),
        ..Layers::default()
    };
    let resolved = merge(&items(), &layers, &no_env);
    assert_eq!(
        resolved.get(key),
        Some((&Value::Bool(true), &line(Layer::System, 1))),
        "宽的不算"
    );
    assert_eq!(
        resolved
            .problems
            .iter()
            .map(|p| (p.code, p.current.clone(), p.got.clone()))
            .collect::<Vec<_>>(),
        [(
            Code::NotTightening,
            Some(Value::Bool(true)),
            Some("false".to_string())
        )]
    );

    let resolved = merge(
        &items(),
        &Layers {
            project: Some((&off, Trust::Trusted)),
            ..Layers::default()
        },
        &no_env,
    );
    assert_eq!(
        resolved.get(key),
        Some((&Value::Bool(false), &line(Layer::Project, 2))),
        "一样的收"
    );
    assert!(resolved.problems.is_empty());
}

#[test]
fn an_untrusted_project_does_not_count() {
    let on = read(Layer::Project, "permission.start_read_only = true\n");
    let key = "permission.start_read_only";
    for (trust, warned) in [(Trust::Unknown, true), (Trust::Distrusted, false)] {
        let layers = Layers {
            project: Some((&on, trust)),
            ..Layers::default()
        };
        let resolved = merge(&items(), &layers, &no_env);
        assert_eq!(
            resolved.get(key),
            Some((&Value::Bool(false), &Origin::Default)),
            "{trust:?}"
        );
        let codes: Vec<Code> = resolved.problems.iter().map(|p| p.code).collect();
        assert_eq!(
            codes == [Code::UntrustedProject],
            warned,
            "{trust:?}：选了不信任的不再提醒"
        );
    }
}

#[test]
fn a_project_cannot_write_what_it_may_not() {
    let project = read(Layer::Project, "ui.language = \"zh\"\n");
    let layers = Layers {
        project: Some((&project, Trust::Trusted)),
        ..Layers::default()
    };
    let resolved = merge(&items(), &layers, &no_env);
    assert_eq!(
        resolved.get("ui.language"),
        Some((&text("auto"), &Origin::Default))
    );
}

#[test]
fn every_layer_is_listed_top_down() {
    let system = read(Layer::System, "\n\n\n\n[ui]\nlanguage = \"en\"\n");
    let personal = read(Layer::Personal, "\n\n[ui]\nlanguage = \"zh\"\n");
    let project = read(
        Layer::Project,
        "ui.language = \"en\"\npermission.start_read_only = false\n",
    );
    let system_on = read(Layer::System, "permission.start_read_only = true\n");
    let layers = Layers {
        system: Some(&system),
        personal: Some(&personal),
        project: Some((&project, Trust::Trusted)),
    };
    let resolved = merge(&items(), &layers, &no_env);
    let listed = explain(&items()[0], &layers, &resolved);
    let row = |origin, value: &'static str, used, problem| Written {
        origin,
        value: text(value),
        used,
        problem,
    };
    assert_eq!(
        listed,
        [
            row(line(Layer::Project, 1), "en", false, Some(Code::WrongLayer)),
            row(line(Layer::Personal, 4), "zh", true, None),
            row(line(Layer::System, 6), "en", false, None),
            row(Origin::Default, "auto", false, None),
        ]
    );
    let layers = Layers {
        system: Some(&system_on),
        personal: None,
        project: Some((&project, Trust::Trusted)),
    };
    let resolved = merge(&items(), &layers, &no_env);
    let listed = explain(&items()[2], &layers, &resolved);
    assert_eq!(
        listed
            .iter()
            .map(|w| (w.origin.layer_name(), w.used, w.problem))
            .collect::<Vec<_>>(),
        [
            ("project", false, Some(Code::NotTightening)),
            ("system", true, None),
            ("default", false, None)
        ]
    );
    let layers = Layers {
        project: Some((&project, Trust::Unknown)),
        ..layers
    };
    let resolved = merge(&items(), &layers, &no_env);
    assert_eq!(
        explain(&items()[2], &layers, &resolved)[0].problem,
        Some(Code::UntrustedProject)
    );
}

#[test]
fn an_environment_variable_is_listed_on_top() {
    let env: BTreeMap<&str, String> = [("MIYU_LOG", "debug".to_string())].into();
    let lookup = |name: &str| env.get(name).cloned();
    let resolved = merge(&items(), &Layers::default(), &lookup);
    let listed = explain(&items()[1], &Layers::default(), &resolved);
    assert_eq!(
        listed
            .iter()
            .map(|w| (w.origin.clone(), w.used))
            .collect::<Vec<_>>(),
        [(Origin::Env("MIYU_LOG"), true), (Origin::Default, false)]
    );
}
