//! 资源里的字和清单对不对得上；几个里的一个怎么连；一项说明后面那几句。

use crate::item::Item;
use crate::test_support::{item, said, system_only, words};
use crate::words::{ConfigWords, Missing, check, facts, one_of};

fn items() -> Vec<Item> {
    vec![
        item("ui.language", &["auto", "zh"], "auto"),
        Item {
            ui: crate::item::Ui {
                page: "advanced",
                group: "log",
                ..item("x.y", &["a", "b"], "a").ui
            },
            ..item("log.level", &["info", "off"], "info")
        },
    ]
}

/// 和 `items()` 对得上的一份。
fn good() -> ConfigWords {
    ConfigWords {
        items: [
            (
                "ui.language".to_string(),
                said("界面语言", "说明。", &["auto", "zh"]),
            ),
            (
                "log.level".to_string(),
                said("日志", "说明。", &["info", "off"]),
            ),
        ]
        .into_iter()
        .collect(),
        pages: [("general", "通用"), ("advanced", "高级")]
            .into_iter()
            .map(|(id, name)| (id.to_string(), name.to_string()))
            .collect(),
        groups: [("display", "显示"), ("log", "运行日志")]
            .into_iter()
            .map(|(id, name)| (id.to_string(), name.to_string()))
            .collect(),
    }
}

#[test]
fn words_that_match_the_list_have_no_problems() {
    assert_eq!(check(&items(), &good()), Vec::<String>::new());
}

#[test]
fn every_item_needs_a_name_a_description_and_option_names() {
    let mut words = good();
    words.items.remove("log.level");
    assert_eq!(check(&items(), &words), vec!["log.level：没有名字和说明"]);

    let mut words = good();
    if let Some(said) = words.items.get_mut("ui.language") {
        said.name = " ".to_string();
        said.description = String::new();
        said.options.remove("zh");
        said.options.insert("auto".to_string(), String::new());
    }
    assert_eq!(
        check(&items(), &words),
        vec![
            "ui.language：名字是空的",
            "ui.language：说明是空的",
            "ui.language：选项 auto 没有名字",
            "ui.language：选项 zh 没有名字",
        ]
    );
}

#[test]
fn words_for_things_not_in_the_list_are_problems() {
    let mut words = good();
    words
        .items
        .insert("ui.old".to_string(), said("旧的", "说明。", &[]));
    if let Some(said) = words.items.get_mut("log.level") {
        said.options
            .insert("verbose".to_string(), "啰嗦".to_string());
    }
    words
        .pages
        .insert("permissions".to_string(), "权限".to_string());
    words
        .groups
        .insert("sessions".to_string(), "会话".to_string());
    assert_eq!(
        check(&items(), &words),
        vec![
            "log.level：资源里多了选项 verbose",
            "ui.old：资源里多了这一项，清单里没有",
            "页 permissions：资源里多了，清单里没有用到",
            "组 sessions：资源里多了，清单里没有用到",
        ]
    );
}

#[test]
fn every_page_and_group_in_use_needs_a_name() {
    let mut words = good();
    words.pages.remove("advanced");
    words.groups.insert("display".to_string(), String::new());
    assert_eq!(
        check(&items(), &words),
        vec!["页 advanced：没有名字", "组 display：没有名字"]
    );
}

#[test]
fn one_of_joins_like_the_command_line_does() {
    let words = words(&[]);
    let values = |parts: &[&str]| one_of(&words, parts, "config/or-values");
    assert_eq!(values(&[]).as_deref(), Ok(""));
    assert_eq!(values(&["a"]).as_deref(), Ok("a"));
    assert_eq!(values(&["a", "b"]).as_deref(), Ok("a 或 b"));
    assert_eq!(values(&["a", "b", "c", "d"]).as_deref(), Ok("a、b、c 或 d"));
    assert_eq!(
        one_of(&words, &["甲", "乙", "丙"], "config/or").as_deref(),
        Ok("甲、乙或丙"),
        "字和字之间不空格"
    );
}

#[test]
fn facts_say_values_layers_and_timing() {
    let level = system_only(item("log.level", &["error", "warn", "info"], "info"));
    let language = item("ui.language", &["auto", "zh"], "auto");
    let words = words(&[]);
    assert_eq!(
        facts(&words, &level).as_deref(),
        Ok("能写：error、warn 或 info。只能写在系统配置里。当场生效。")
    );
    assert_eq!(
        facts(&words, &language).as_deref(),
        Ok("能写：auto 或 zh。只能写在系统配置或个人设置里。当场生效。")
    );
}

#[test]
fn missing_sentences_are_named() {
    let level = item("log.level", &["error", "warn", "info"], "info");
    for key in [
        "config/list",
        "config/or-values",
        "config/layer/personal",
        "config/or",
        "config/applies/now",
        "config/facts",
    ] {
        let mut words = words(&[]);
        words.sentences.remove(key);
        assert_eq!(
            facts(&words, &level),
            Err(Missing(key.to_string())),
            "{key}"
        );
    }
    assert_eq!(
        Missing("config/or".to_string()).to_string(),
        "no words for config/or"
    );
}
