//! `settings!` 生成的清单和设置类型一一对上；选项的校验。没写默认值、选项只有一个的编译不过，写在宏的文档里
//! （`compile_fail` 的例子，不引 `trybuild`）。

use std::borrow::Cow;

use crate::item::{Applies, Control, Item, Kind, Layer, Ui};
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
    assert_eq!(Applies::Now.as_str(), "now");
    assert_eq!(Control::Select.as_str(), "select");
}
