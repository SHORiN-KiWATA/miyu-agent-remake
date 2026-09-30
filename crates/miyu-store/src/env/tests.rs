//! 系统的语言：`LC_ALL`、`LC_MESSAGES`、`LANG` 的先后，空的当没设。

use std::collections::BTreeMap;

use super::*;

/// 照 `vars` 读的系统语言。
fn locale_of(vars: &[(&str, &str)]) -> Option<String> {
    let vars: BTreeMap<&str, &str> = vars.iter().copied().collect();
    locale_from(|name| vars.get(name).map(|value| value.to_string()))
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
