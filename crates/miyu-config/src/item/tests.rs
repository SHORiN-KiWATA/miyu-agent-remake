//! `settings!` 生成的清单和设置类型一一对上；选项的校验。没写默认值、选项只有一个的编译不过，写在宏的文档里
//! （`compile_fail` 的例子，不引 `trybuild`）。

use std::borrow::Cow;

use crate::item::{Applies, Control, Item, Kind, Layer, Tighten, Ui};
use crate::value::{Value, Values};

crate::settings! {
    /// 测试用的设置。
    pub struct Sample in "sample" {
        /// 第一项：常用，能放两层。
        first: String = "b" {
            kind: option ["a", "b"],
            layers: [System, Personal],
            applies: now,
            ui: { page: "general", group: "display", common: true, control: select },
        },
        /// 第二项：有环境变量压着，只能放系统配置。
        second: String = "x" {
            kind: option ["x", "y", "z"],
            layers: [System],
            env: "MIYU_SAMPLE",
            applies: now,
            ui: { page: "advanced", group: "log", control: select },
        },
    }
}

crate::settings! {
    /// 测试用的开关（施工 8-2）：三层都能放，项目配置只能打开它，以后开的会话生效。
    pub struct Switches in "switch" {
        /// 一个开关。
        start_read_only: bool = false {
            kind: bool,
            layers: [System, Personal, Project],
            tighten: true_only,
            applies: new_session,
            ui: { page: "permissions", group: "sessions", control: toggle },
        },
    }
}

#[test]
fn a_switch_is_declared_with_how_a_project_tightens_it() {
    assert_eq!(
        Switches::ITEMS,
        [Item {
            key: "switch.start_read_only",
            kind: Kind::Bool,
            default: Value::Bool(false),
            layers: &[Layer::System, Layer::Personal, Layer::Project],
            tighten: Some(Tighten::TrueOnly),
            env: None,
            applies: Applies::NewSession,
            ui: Ui {
                page: "permissions",
                group: "sessions",
                common: false,
                control: Control::Toggle,
            },
        }]
    );
    assert!(!Switches::from(&Values::default()).start_read_only);
    let mut values = Values::default();
    values.set("switch.start_read_only", Value::Bool(true));
    assert!(Switches::from(&values).start_read_only, "照最终值");
}

#[test]
fn a_switch_accepts_only_true_and_false() {
    assert!(Kind::Bool.accepts(&Value::Bool(true)));
    assert!(Kind::Bool.accepts(&Value::Bool(false)));
    assert!(!Kind::Bool.accepts(&Value::Text(Cow::Borrowed("true"))));
    assert!(!Kind::Option(&["a", "b"]).accepts(&Value::Bool(true)));
}

#[test]
fn only_a_looser_project_value_is_refused() {
    let (on, off) = (Value::Bool(true), Value::Bool(false));
    assert!(Tighten::TrueOnly.looser(&off, &on), "关掉更宽");
    assert!(!Tighten::TrueOnly.looser(&on, &off), "打开更严");
    assert!(!Tighten::TrueOnly.looser(&on, &on), "一样的不算宽");
    assert!(!Tighten::TrueOnly.looser(&off, &off));
}

#[test]
fn environment_values_are_read_loosely() {
    let level = Kind::Option(&["info", "debug"]);
    assert_eq!(
        level.from_env(" DEBUG "),
        Some(Value::Text(Cow::Borrowed("debug"))),
        "不分大小写、去空白"
    );
    assert_eq!(level.from_env("loud"), None);
    assert_eq!(Kind::Bool.from_env("TRUE"), Some(Value::Bool(true)));
    assert_eq!(Kind::Bool.from_env("yes"), None);
}

#[test]
fn the_items_follow_the_fields_in_order() {
    assert_eq!(
        Sample::ITEMS,
        [
            Item {
                key: "sample.first",
                kind: Kind::Option(&["a", "b"]),
                default: Value::Text(Cow::Borrowed("b")),
                layers: &[Layer::System, Layer::Personal],
                tighten: None,
                env: None,
                applies: Applies::Now,
                ui: Ui {
                    page: "general",
                    group: "display",
                    common: true,
                    control: Control::Select,
                },
            },
            Item {
                key: "sample.second",
                kind: Kind::Option(&["x", "y", "z"]),
                default: Value::Text(Cow::Borrowed("x")),
                layers: &[Layer::System],
                tighten: None,
                env: Some("MIYU_SAMPLE"),
                applies: Applies::Now,
                ui: Ui {
                    page: "advanced",
                    group: "log",
                    common: false,
                    control: Control::Select,
                },
            },
        ]
    );
}

#[test]
fn the_settings_come_from_the_values() {
    assert_eq!(
        Sample::from(&Values::defaults(Sample::ITEMS)),
        Sample {
            first: "b".to_string(),
            second: "x".to_string(),
        }
    );
    // 最终值里有的照它，不照写在代码里的默认值：拿一份改了默认值的清单造最终值。
    let changed = [Item {
        default: Value::Text(Cow::Borrowed("z")),
        ..Sample::ITEMS[1].clone()
    }];
    assert_eq!(
        Sample::from(&Values::defaults(&changed)),
        Sample {
            first: "b".to_string(),
            second: "z".to_string(),
        },
        "有的照最终值，没有的照默认值"
    );
    assert_eq!(
        Sample::from(&Values::default()),
        Sample::from(&Values::defaults(Sample::ITEMS)),
        "什么都没有的照默认值"
    );
}

#[test]
fn an_option_accepts_only_the_listed_ones_with_case() {
    let kind = Kind::Option(&["info", "debug"]);
    let text = |text: &'static str| Value::Text(Cow::Borrowed(text));
    assert!(kind.accepts(&text("info")));
    assert!(kind.accepts(&text("debug")));
    assert!(!kind.accepts(&text("Info")), "区分大小写");
    assert!(!kind.accepts(&text("")));
    assert!(!kind.accepts(&text("verbose")));
}

#[test]
fn layers_and_timings_are_written_as_on_the_wire() {
    assert_eq!(Layer::System.as_str(), "system");
    assert_eq!(Layer::Personal.as_str(), "personal");
    assert_eq!(Layer::Project.as_str(), "project");
    assert_eq!(Applies::Now.as_str(), "now");
    assert_eq!(Applies::NewSession.as_str(), "new_session");
    assert_eq!(Control::Select.as_str(), "select");
    assert_eq!(Control::Toggle.as_str(), "toggle");
    assert_eq!(Tighten::TrueOnly.as_str(), "true_only");
    assert_eq!(Kind::Bool.as_str(), "bool");
    assert_eq!(Kind::Option(&["a", "b"]).as_str(), "option");
}
