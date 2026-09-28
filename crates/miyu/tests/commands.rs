//! 命令行的规矩（`docs/designs/22-命令行.md` 第二节，施工 3-9 上）：不认识的子命令就报错、退出码 2，绝不当成
//! 对话发给核心；给人看的话跟着界面语言。

mod support;

use std::process::{Command, Output};

use miyu_cli::help::{Page, page};
use miyu_cli::language::Language;
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
fn the_help_is_the_page_and_the_version_says_miyu() {
    let home = Home::new();
    // `-h`、`--help`、`help` 印的都是自己写的那一页，一个字节不差（施工 4-11）。
    for (lang, language) in [("zh_CN.UTF-8", Language::Chinese), ("C", Language::English)] {
        for args in [&["-h"][..], &["--help"], &["help"]] {
            let output = miyu(&home, lang, args);
            assert!(output.status.success(), "{args:?}：{output:?}");
            assert_eq!(
                String::from_utf8_lossy(&output.stdout),
                page(language, Page::Miyu),
                "{lang} {args:?}"
            );
            assert!(output.stderr.is_empty(), "{output:?}");
        }
    }
    let output = miyu(&home, "C", &["--version"]);
    assert!(output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stdout).starts_with("miyu "));
}

#[test]
fn a_mistake_is_one_sentence_and_exit_code_2() {
    let home = Home::new();
    let cases: [(&[&str], &str, &str); 4] = [
        (&["ask"], "zh_CN.UTF-8", "少了要说的话：miyu ask \"…\""),
        (
            &["ask", "--format", "xml", "hi"],
            "C",
            "--format must be text or json",
        ),
        // 短写的 `-s`、`-c` 就是 `--session`、`--continue`。
        (
            &["ask", "-s", "x", "-c", "hi"],
            "zh_CN.UTF-8",
            "--session 和 --continue 只能写一个",
        ),
        (&["undo", "-s"], "zh_CN.UTF-8", "--session 后面少了值"),
    ];
    for (args, lang, said) in cases {
        let output = miyu(&home, lang, args);
        assert_eq!(output.status.code(), Some(2), "{args:?}：{output:?}");
        assert_eq!(
            String::from_utf8_lossy(&output.stderr),
            format!("{said}\n"),
            "{args:?}"
        );
        assert!(output.stdout.is_empty(), "{output:?}");
    }
}
