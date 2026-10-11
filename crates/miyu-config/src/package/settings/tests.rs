//! 清单里的 `[settings]`（施工 9-1 下）：每一种类型读对、默认值照类型查、几层、什么时候生效、名字和说明、隐藏；每一种写错报
//! 对代码；转成配置项以后键、类型、界面提示对。

use super::*;
use crate::package::{Code, read};

/// 一份带配置项的网页清单。
fn web(settings: &str) -> String {
    format!(
        "[package]\nprotocol = [1, 1]\nname = {{ en = \"Web\", zh = \"网页\" }}\n\n{settings}\n\n[ui]\n"
    )
}

const PORT: &str = r#"[settings.port]
type = "int"
min = 1
max = 65535
default = 8300
layers = ["system"]
name = { en = "Port", zh = "端口" }
description = { en = "Which port the web interface listens on." }
"#;

fn setting(text: &str) -> Setting {
    let mut settings = read(&web(text)).unwrap().settings;
    assert_eq!(settings.len(), 1);
    settings.remove(0)
}

fn wrong(text: &str) -> Code {
    read(&web(text)).expect_err("应该读不成").code
}

#[test]
fn an_int_reads_its_range_default_layers_and_words() {
    let port = setting(PORT);
    assert_eq!(port.name, "port");
    assert_eq!(port.kind, SettingKind::Int { min: 1, max: 65535 });
    assert_eq!(port.default, Some(Value::Int(8300)));
    assert_eq!(port.layers, [Layer::System]);
    assert_eq!(port.applies, Applies::HeadStart, "不写是这个程序下次启动时");
    assert_eq!(port.label.get("zh").map(String::as_str), Some("端口"));
    assert_eq!(port.description.len(), 1);
    assert!(!port.hidden);
}

#[test]
fn every_type_reads() {
    let cases = [
        (
            "type = \"bool\"\ndefault = true",
            SettingKind::Bool,
            Some(Value::Bool(true)),
        ),
        (
            "type = \"option\"\nchoices = [\"a\", \"b\"]\ndefault = \"b\"",
            SettingKind::Option(vec!["a".to_string(), "b".to_string()]),
            Some(Value::Text("b".into())),
        ),
        (
            "type = \"text\"\nmax = 20",
            SettingKind::Text { max: 20 },
            None,
        ),
        ("type = \"text\"", SettingKind::Text { max: 200 }, None),
        (
            "type = \"name\"\ndefault = \"miyu\"",
            SettingKind::Name,
            Some(Value::Text("miyu".into())),
        ),
        ("type = \"url\"", SettingKind::Url, None),
        ("type = \"secret\"", SettingKind::Secret, None),
        (
            "type = \"int\"",
            SettingKind::Int {
                min: i64::MIN,
                max: i64::MAX,
            },
            None,
        ),
    ];
    for (body, kind, default) in cases {
        let one = setting(&format!("[settings.x]\n{body}\nname = {{ en = \"X\" }}\n"));
        assert_eq!((one.kind, one.default), (kind, default), "{body}");
    }
}

#[test]
fn a_list_reads_its_element_and_checks_the_default_one_by_one() {
    let list = |kind: SettingKind| SettingKind::List(Box::new(kind));
    let text = |text: &str| Value::Text(text.to_string().into());
    let cases = [
        (
            "element = \"text\"\ndefault = [\"qq:1\", \"qq:2\"]",
            list(SettingKind::Text { max: 200 }),
            Some(Value::List(vec![text("qq:1"), text("qq:2")])),
        ),
        (
            "element = \"int\"\nmin = 1\nmax = 9\ndefault = [1, 9]",
            list(SettingKind::Int { min: 1, max: 9 }),
            Some(Value::List(vec![Value::Int(1), Value::Int(9)])),
        ),
        (
            "element = \"option\"\nchoices = [\"a\", \"b\"]\ndefault = [\"b\"]",
            list(SettingKind::Option(vec!["a".to_string(), "b".to_string()])),
            Some(Value::List(vec![text("b")])),
        ),
        (
            "element = \"text\"\nmax = 8\ndefault = []",
            list(SettingKind::Text { max: 8 }),
            Some(Value::List(Vec::new())),
        ),
        ("element = \"bool\"", list(SettingKind::Bool), None),
        ("element = \"name\"", list(SettingKind::Name), None),
        ("element = \"url\"", list(SettingKind::Url), None),
        ("element = \"secret\"", list(SettingKind::Secret), None),
    ];
    for (body, kind, default) in cases {
        let one = setting(&format!(
            "[settings.x]\ntype = \"list\"\n{body}\nname = {{ en = \"X\" }}\n"
        ));
        assert_eq!((one.kind, one.default), (kind, default), "{body}");
    }
}

#[test]
fn every_wrong_list_says_what() {
    let x =
        |body: &str| format!("[settings.x]\ntype = \"list\"\n{body}\nname = {{ en = \"X\" }}\n");
    assert_eq!(wrong(&x("")), Code::MissingKey, "少了 element");
    assert_eq!(wrong(&x("element = \"list\"")), Code::BadElement);
    assert_eq!(wrong(&x("element = \"float\"")), Code::BadElement);
    assert_eq!(wrong(&x("element = 1")), Code::BadElement);
    assert_eq!(
        wrong(&x("element = \"int\"\nmax = 9\ndefault = [1, 10]")),
        Code::BadDefault,
        "每一个照元素查"
    );
    assert_eq!(
        wrong(&x("element = \"int\"\ndefault = 1")),
        Code::BadDefault,
        "不是数组"
    );
    assert_eq!(
        wrong(&x("element = \"secret\"\ndefault = []")),
        Code::BadDefault,
        "密钥的列表不能有默认值"
    );
    assert_eq!(
        wrong(&x("element = \"option\"\nchoices = [\"a\"]")),
        Code::BadChoices
    );
    assert_eq!(
        wrong(&x(
            "element = \"option\"\nchoices = [\"a\", \"b\"]\ndefault = [\"a\", \"c\"]"
        )),
        Code::BadDefault
    );
    assert_eq!(
        wrong(&x("element = \"bool\"\nchoices = [\"a\", \"b\"]")),
        Code::UnknownKey,
        "元素带的几格照元素的类型收"
    );
    assert_eq!(
        wrong("[settings.x]\ntype = \"text\"\nelement = \"text\"\nname = { en = \"X\" }\n"),
        Code::UnknownKey,
        "element 只有列表能写"
    );
}

#[test]
fn layers_applies_and_hidden_read() {
    let one = setting(
        "[settings.idle_seconds]\ntype = \"int\"\nlayers = [\"system\", \"personal\"]\napplies = \"now\"\nhidden = true\nname = { en = \"Idle\" }\n",
    );
    assert_eq!(one.layers, [Layer::System, Layer::Personal]);
    assert_eq!(one.applies, Applies::Now);
    assert!(one.hidden);
    let both = setting("[settings.x]\ntype = \"bool\"\nname = { en = \"X\" }\n");
    assert_eq!(
        both.layers,
        [Layer::System, Layer::Personal],
        "不写是两层都能写"
    );
    for (applies, expected) in [
        ("new_session", Applies::NewSession),
        ("next_turn", Applies::NextTurn),
        ("program_start", Applies::HeadStart),
    ] {
        let one = setting(&format!(
            "[settings.x]\ntype = \"bool\"\napplies = \"{applies}\"\nname = {{ en = \"X\" }}\n"
        ));
        assert_eq!(one.applies, expected);
    }
}

#[test]
fn every_wrong_setting_says_what() {
    let x = |body: &str| format!("[settings.x]\n{body}\nname = {{ en = \"X\" }}\n");
    assert_eq!(
        wrong("[settings.Bad]\ntype = \"bool\"\nname = { en = \"X\" }\n"),
        Code::BadSettingName
    );
    assert_eq!(
        wrong("[settings.x]\nname = { en = \"X\" }\n"),
        Code::MissingKey
    );
    assert_eq!(wrong("[settings.x]\ntype = \"bool\"\n"), Code::MissingKey);
    assert_eq!(wrong(&x("type = \"float\"")), Code::BadType);
    assert_eq!(
        wrong(&x("type = \"int\"\ndefault = \"8300\"")),
        Code::BadDefault
    );
    assert_eq!(
        wrong(&x("type = \"int\"\nmin = 1\nmax = 10\ndefault = 11")),
        Code::BadDefault
    );
    assert_eq!(
        wrong(&x("type = \"int\"\nmin = 10\nmax = 1")),
        Code::BadRange
    );
    assert_eq!(
        wrong(&x("type = \"option\"\nchoices = [\"a\"]")),
        Code::BadChoices
    );
    assert_eq!(wrong(&x("type = \"option\"")), Code::BadChoices);
    assert_eq!(
        wrong(&x("type = \"option\"\nchoices = [\"a\", \"b\", \"a\"]")),
        Code::BadChoices,
        "有重复的"
    );
    assert_eq!(
        wrong(&x(
            "type = \"option\"\nchoices = [\"a\", \"b\"]\ndefault = \"c\""
        )),
        Code::BadDefault
    );
    assert_eq!(
        wrong(&x("type = \"bool\"\nchoices = [\"a\", \"b\"]")),
        Code::UnknownKey
    );
    assert_eq!(
        wrong(&x("type = \"bool\"\nlayers = [\"project\"]")),
        Code::BadLayers
    );
    assert_eq!(wrong(&x("type = \"bool\"\nlayers = []")), Code::BadLayers);
    assert_eq!(
        wrong(&x("type = \"bool\"\napplies = \"later\"")),
        Code::BadApplies
    );
    assert_eq!(
        wrong(&x("type = \"bool\"\nhidden = \"yes\"")),
        Code::NotBool
    );
    assert_eq!(
        wrong(&x("type = \"secret\"\ndefault = \"sk\"")),
        Code::BadDefault,
        "密钥不能有默认值"
    );
    assert_eq!(wrong(&x("type = \"bool\"\ncolour = 1")), Code::UnknownKey);
    assert_eq!(wrong("[settings]\nx = 1\n"), Code::NotATable);
}

#[test]
fn settings_become_config_items_under_the_package() {
    let manifest = read(&web(&format!(
        "{PORT}\n[settings.idle_seconds]\ntype = \"int\"\nmin = 1\ndefault = 600\nhidden = true\nname = {{ en = \"Idle\" }}\n"
    )))
    .unwrap();
    let items = items("web", &manifest.settings);
    assert_eq!(items.len(), 2);
    let port = &items[0];
    assert_eq!(port.key, "web.port");
    assert_eq!(port.kind, Kind::Int { min: 1, max: 65535 });
    assert_eq!(port.default, Some(Value::Int(8300)));
    assert_eq!(port.layers, [Layer::System]);
    assert_eq!(port.applies, Applies::HeadStart);
    assert_eq!(port.tighten, None);
    assert_eq!(
        (port.ui.page, port.ui.group, port.ui.control, port.ui.hidden),
        (PAGE, "web", Control::Number, false)
    );
    assert!(items[1].ui.hidden);
    let option = read(&web(
        "[settings.mode]\ntype = \"option\"\nchoices = [\"a\", \"b\"]\nname = { en = \"Mode\" }\n",
    ))
    .unwrap();
    let item = &items_of(&option)[0];
    assert_eq!(item.kind, Kind::Option(&["a", "b"]));
    assert_eq!(item.ui.control, Control::Select);
    let lists = read(&web(
        "[settings.trusted]
type = \"list\"\nelement = \"text\"\nname = { en = \"Trusted\" }\n\n[settings.modes]\ntype = \"list\"\nelement = \"option\"\nchoices = [\"a\", \"b\"]\nname = { en = \"Modes\" }\n",
    ))
    .unwrap();
    let items = items_of(&lists);
    assert_eq!(items[0].kind, Kind::List(&Kind::Text { max: 200 }));
    assert_eq!(items[0].ui.control, Control::List);
    assert_eq!(items[1].kind, Kind::List(&Kind::Option(&["a", "b"])));
    assert_eq!(items[1].ui.control, Control::List);
}

fn items_of(manifest: &crate::package::Manifest) -> Vec<Item> {
    items("x", &manifest.settings)
}
