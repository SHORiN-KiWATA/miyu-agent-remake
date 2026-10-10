//! 伪终端里换吉祥物（蓝图 `tui.md`「吉祥物包」第 5 条）：`MIYU_TUI_MASCOT` 指一份本机的模型文件，首页照它画；写错的照
//! 内置的画，提示一句带原因。

mod support;

use std::time::Duration;

use miyu_session::testkit::Script;
use support::Home;

/// 写一份模型文件，起界面停在首页。
fn home_with_model(model: &str) -> (Home, support::Tui) {
    let home = Home::new(Script::new([]));
    let path = home.work.join("mascot.json");
    std::fs::write(&path, model).expect("写模型文件");
    let path = path.display().to_string();
    let mut tui = home.tui_with("zh_CN.UTF-8", &[("MIYU_TUI_MASCOT", path.as_str())]);
    tui.wait_for("Tab 切换权限级别");
    tui.pump(Duration::from_millis(300));
    (home, tui)
}

#[test]
fn a_local_model_draws_the_mascot_on_the_home_screen() {
    // 只换了明暗的那串字：头整个照 `x` 画，内置的 `%`、`@` 一个都没有。
    let (_home, tui) = home_with_model(r#"{"ramp": " xxxxxxxxx"}"#);
    let screen = tui.lines().join("\n");
    assert!(screen.contains("xxxx"), "照本机的模型画：\n{screen}");
    assert!(
        !screen.contains('%') && !screen.contains('@'),
        "不是内置的：\n{screen}"
    );
}

#[test]
fn a_broken_model_falls_back_to_the_built_in_mascot_and_says_why() {
    let (_home, tui) = home_with_model(r#"{"cols": 99}"#);
    let screen = tui.lines().join("\n");
    assert!(screen.contains("吉祥物包无法读取"), "提示一句：\n{screen}");
    assert!(screen.contains("cols"), "本机试画的带原因：\n{screen}");
    assert!(screen.contains('%'), "照内置的画：\n{screen}");
}
