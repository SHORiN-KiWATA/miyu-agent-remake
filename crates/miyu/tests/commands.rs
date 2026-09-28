//! 命令行的规矩（`docs/designs/22-命令行.md` 第二节，施工 3-9 上）：不认识的子命令就报错、退出码 2，绝不当成
//! 对话发给核心；给人看的话跟着界面语言。

mod support;

use std::process::{Command, Output};

use support::{Home, MIYU};

/// 在临时的数据根上跑 `miyu <args>`，界面语言是 `lang`。
fn miyu(home: &Home, lang: &str, args: &[&str]) -> Output {
    Command::new(MIYU)
        .args(args)
        .env("MIYU_HOME", home.root.path())
        .env("LANG", lang)
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .output()
        .expect("跑得起来")
}

#[test]
fn an_unknown_command_is_refused_and_never_sent() {
    let home = Home::new();
    let output = miyu(&home, "zh_CN.UTF-8", &["hello"]);
    assert_eq!(output.status.code(), Some(2));
    let said = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        said.trim_end(),
        "没有 hello 这个子命令。想和她对话，用 miyu ask \"…\""
    );
    assert!(output.stdout.is_empty());
    assert!(!home.root.run().join("socket").exists(), "没拉起核心");
    assert!(home.core_log().is_empty(), "核心没起来过");

    let output = miyu(&home, "C", &["hello"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        String::from_utf8_lossy(&output.stderr).trim_end(),
        "There is no hello command. To talk to her, use miyu ask \"…\""
    );
}

#[test]
fn plain_miyu_says_what_to_use_for_now() {
    let home = Home::new();
    let output = miyu(&home, "zh_CN.UTF-8", &[]);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("miyu ask"),
        "{output:?}"
    );
}

#[test]
fn help_and_version_are_fine() {
    let home = Home::new();
    for args in [["--help"], ["--version"]] {
        let output = miyu(&home, "C", &args);
        assert!(output.status.success(), "{args:?}：{output:?}");
    }
    let output = miyu(&home, "C", &["--version"]);
    assert!(String::from_utf8_lossy(&output.stdout).starts_with("miyu "));
    // 帮助直接从用法开始，不印代码注释（施工 4-9 再补四上）。
    for language in ["C", "zh_CN.UTF-8"] {
        let output = miyu(&home, language, &["--help"]);
        let help = String::from_utf8_lossy(&output.stdout);
        assert!(help.starts_with("Usage: miyu"), "{language}：{help}");
        assert!(!help.contains('`'), "{language}：{help}");
    }
}
