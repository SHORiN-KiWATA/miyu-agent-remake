//! 查清单：每一种不对各一例，对的一份一处都不报。

use std::borrow::Cow;

use crate::item::{Item, Kind, Layer};
use crate::list::check;
use crate::test_support::item;
use crate::value::Value;

/// 两项都对的一份。
fn good() -> Vec<Item> {
    vec![
        item("ui.language", &["auto", "zh"], "auto"),
        item("log.level", &["info", "off"], "info"),
    ]
}

/// 只查一项，交回的每一句都带着它的键。
fn one(item: Item) -> Vec<String> {
    check(&[item])
}

#[test]
fn a_good_list_has_no_problems() {
    assert_eq!(check(&good()), Vec::<String>::new());
    assert_eq!(check(&[]), Vec::<String>::new());
    assert!(
        one(item("a1.b_2.c3", &["x", "y"], "x")).is_empty(),
        "数字、_、三段都行"
    );
}

#[test]
fn the_same_key_twice_is_a_problem() {
    let mut items = good();
    items.push(item("ui.language", &["en", "ja"], "en"));
    let problems = check(&items);
    assert_eq!(problems, vec!["ui.language：键重复了"], "{problems:?}");
}

#[test]
fn a_key_that_is_a_prefix_of_another_is_a_problem() {
    let mut items = good();
    items.push(item("ui.language.extra", &["a", "b"], "a"));
    let problems = check(&items);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(
        problems[0].starts_with("ui.language.extra：ui.language 是它的前缀"),
        "{problems:?}"
    );
    // 照段比，不照字比：`ui.lang` 不是 `ui.language` 的前缀。
    let mut items = good();
    items.push(item("ui.lang", &["a", "b"], "a"));
    assert!(check(&items).is_empty());
}

#[test]
fn keys_are_written_the_right_way() {
    for bad in [
        "ui",
        "Ui.language",
        "ui.Language",
        "ui.langUage",
        "ui.lang-uage",
        "ui..language",
        "ui.1st",
        "ui.",
        ".ui",
        "ui._x",
    ] {
        let problems = one(item(bad, &["a", "b"], "a"));
        assert_eq!(problems.len(), 1, "{bad}：{problems:?}");
        assert!(problems[0].starts_with(&format!("{bad}：")), "{problems:?}");
    }
    let problems = one(item("ext.weather.units", &["a", "b"], "a"));
    assert_eq!(problems, vec!["ext.weather.units：第一段 ext 留给扩展"]);
}

#[test]
fn the_default_has_to_pass_its_own_check() {
    let problems = one(item("log.level", &["info", "off"], "verbose"));
    assert_eq!(
        problems,
        vec![r#"log.level：默认值 "verbose" 过不了自己的校验"#]
    );
}

#[test]
fn an_option_needs_two_different_choices() {
    let problems = one(item("log.level", &["info"], "info"));
    assert_eq!(problems, vec!["log.level：选项至少两个"]);
    let problems = one(item("log.level", &["info", "info"], "info"));
    assert_eq!(problems, vec!["log.level：选项写重了"]);
}

#[test]
fn an_item_needs_one_layer_and_no_layer_twice() {
    let problems = one(Item {
        layers: &[],
        ..item("log.level", &["info", "off"], "info")
    });
    assert_eq!(problems, vec!["log.level：一层都不能放"]);
    let problems = one(Item {
        layers: &[Layer::System, Layer::System],
        ..item("log.level", &["info", "off"], "info")
    });
    assert_eq!(problems, vec!["log.level：层写重了"]);
}

#[test]
fn every_problem_is_reported() {
    let bad = Item {
        kind: Kind::Option(&["a"]),
        default: Some(Value::Text(Cow::Borrowed("b"))),
        layers: &[],
        ..item("Bad", &["a", "b"], "a")
    };
    let problems = check(&[bad.clone(), bad]);
    assert_eq!(
        problems.len(),
        9,
        "每一样都报，两项各报一遍，重复再一句：{problems:?}"
    );
}

#[test]
fn a_project_item_says_how_it_tightens_and_only_then() {
    use crate::item::Tighten;
    let switch = Item {
        kind: Kind::Bool,
        default: Some(Value::Bool(false)),
        layers: &[Layer::System, Layer::Project],
        tighten: Some(Tighten::TrueOnly),
        ..item("permission.start_read_only", &[], "")
    };
    assert_eq!(one(switch.clone()), Vec::<String>::new());
    assert_eq!(
        one(Item {
            tighten: None,
            ..switch.clone()
        }),
        vec!["permission.start_read_only：能放进项目配置，要写怎么收紧"]
    );
    assert_eq!(
        one(Item {
            layers: &[Layer::System],
            ..switch.clone()
        }),
        vec!["permission.start_read_only：不能放进项目配置，不写收紧"]
    );
    assert_eq!(
        one(Item {
            layers: &[Layer::Project],
            tighten: Some(Tighten::TrueOnly),
            ..item("ui.language", &["a", "b"], "a")
        }),
        vec!["ui.language：只能打开只给开关"]
    );
    assert_eq!(
        one(Item {
            default: Some(Value::Text(Cow::Borrowed("false"))),
            ..switch
        }),
        vec!["permission.start_read_only：默认值 \"false\" 过不了自己的校验"]
    );
}
