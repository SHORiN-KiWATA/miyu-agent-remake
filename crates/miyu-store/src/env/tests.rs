//! 系统的语言：`LC_ALL`、`LC_MESSAGES`、`LANG` 的先后，空的当没设；都没设的照系统设置（施工 8-2）。

use std::collections::BTreeMap;

use super::*;

/// 照 `vars` 读的系统语言。
fn locale_of(vars: &[(&str, &str)]) -> Option<String> {
    let vars: BTreeMap<&str, &str> = vars.iter().copied().collect();
    locale_from(
        |name| vars.get(name).map(|value| value.to_string()),
        || None,
    )
}

#[test]
fn the_first_set_variable_wins() {
    let all = [
        ("LC_ALL", "ja_JP.UTF-8"),
        ("LC_MESSAGES", "zh_CN.UTF-8"),
        ("LANG", "en_US.UTF-8"),
    ];
    assert_eq!(locale_of(&all).as_deref(), Some("ja_JP.UTF-8"));
    assert_eq!(locale_of(&all[1..]).as_deref(), Some("zh_CN.UTF-8"));
    assert_eq!(locale_of(&all[2..]).as_deref(), Some("en_US.UTF-8"));
}

#[test]
fn empty_ones_do_not_count() {
    assert_eq!(
        locale_of(&[("LC_ALL", ""), ("LC_MESSAGES", ""), ("LANG", "zh_CN.UTF-8")]).as_deref(),
        Some("zh_CN.UTF-8")
    );
    assert_eq!(locale_of(&[("LC_ALL", "")]), None);
    assert_eq!(
        locale_of(&[("LC_CTYPE", "zh_CN.UTF-8")]),
        None,
        "别的变量不看"
    );
}

#[test]
fn the_system_setting_is_asked_only_when_no_variable_is_set() {
    let vars = |name: &str| (name == "LANG").then(|| "en_US.UTF-8".to_string());
    let asked = std::cell::Cell::new(false);
    let system = || {
        asked.set(true);
        Some("ja-JP".to_string())
    };
    assert_eq!(locale_from(vars, system).as_deref(), Some("en_US.UTF-8"));
    assert!(!asked.get(), "环境变量设了的不看系统设置");
    assert_eq!(
        locale_from(|_| Some(String::new()), || Some("ja-JP".to_string())).as_deref(),
        Some("ja-JP"),
        "都没设（空的也算没设）的照系统设置"
    );
    assert_eq!(locale_from(|_| None, || None), None);
}

/// macOS、Windows 上系统设置总有一种语言（CI 上跑，`config.md`「守着它的」）。
#[cfg(any(target_os = "macos", windows))]
#[test]
fn macos_and_windows_have_a_system_language() {
    let locale = system_locale().expect("系统设置里有语言");
    assert!(
        locale
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic()),
        "BCP 47 的写法：{locale}"
    );
}
