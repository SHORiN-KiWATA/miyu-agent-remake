//! 界面语言：定了的照定的，`auto` 照系统的语言。

use super::*;

fn chosen(language: &str) -> UiSettings {
    UiSettings {
        language: language.to_string(),
        startup: "new".to_string(),
        head: "tui".to_string(),
    }
}

#[test]
fn auto_follows_the_system_language() {
    let auto = UiSettings::from(&miyu_config::Values::defaults(UiSettings::ITEMS));
    assert_eq!(auto.language, "auto", "默认跟着系统");
    assert_eq!(auto.language_for(Some("zh_CN.UTF-8")), "zh");
    assert_eq!(auto.language_for(Some("zh-TW")), "zh");
    assert_eq!(auto.language_for(Some("ja_JP.UTF-8")), "ja");
    assert_eq!(auto.language_for(Some("en_US.UTF-8")), "en");
    assert_eq!(auto.language_for(Some("C.UTF-8")), "en");
    assert_eq!(auto.language_for(Some("")), "en");
    assert_eq!(auto.language_for(None), "en");
}

#[test]
fn a_chosen_language_wins_over_the_system() {
    assert_eq!(chosen("ja").language_for(Some("zh_CN.UTF-8")), "ja");
    assert_eq!(chosen("zh").language_for(None), "zh");
    assert_eq!(chosen("en").language_for(Some("ja_JP.UTF-8")), "en");
}
