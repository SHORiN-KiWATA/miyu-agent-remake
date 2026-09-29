//! 真跑 `miyu ask`（`docs/construction/3-9-miyu-ask（下）.md`）：没有 key、核心也没在跑的，不拉起、退出码 5；
//! 核心在跑的，头没有 key 照样连它；参数不对的退出码 2；帮助页跟着界面语言。

mod support;

use std::process::{Command, Output};

use miyu_cli::help::{Page, page};
use miyu_cli::language::Language;
use miyu_ipc::connect_or_start;
use support::{Home, MIYU, within};

/// 在临时的数据根上跑 `miyu ask <args>`：没有 key，界面语言是 `lang`。
fn ask(home: &Home, lang: &str, args: &[&str]) -> Output {
    Command::new(MIYU)
        .arg("ask")
        .args(args)
        .env("MIYU_HOME", home.root.path())
        .env("MIYU_RESOURCES", support::resources())
        .env("LANG", lang)
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .env_remove("DEEPSEEK_API_KEY")
        .env_remove("XDG_RUNTIME_DIR")
        .output()
        .expect("跑得起来")
}

#[test]
fn without_a_key_and_a_core_nothing_is_started() {
    let home = Home::new();
    let output = ask(&home, "zh_CN.UTF-8", &["在吗"]);
    assert_eq!(output.status.code(), Some(5), "{output:?}");
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "没有可用的模型：设环境变量 DEEPSEEK_API_KEY\n"
    );
    assert!(!home.root.run().join("socket").exists(), "没拉起核心");
    assert!(home.core_log().is_empty());
}

#[tokio::test]
async fn a_running_core_is_used_even_without_a_key_here() {
    let home = Home::new();
    let (held, _) = within("拉起", connect_or_start(&home.root, || home.core()))
        .await
        .expect("拉得起");
    let output = tokio::task::spawn_blocking({
        let home_root = home.root.path().to_path_buf();
        move || {
            Command::new(MIYU)
                .args(["ask", "在吗"])
                .env("MIYU_HOME", home_root)
                .env("LANG", "zh_CN.UTF-8")
                .env_remove("LC_ALL")
                .env_remove("LC_MESSAGES")
                .env_remove("DEEPSEEK_API_KEY")
                .output()
                .expect("跑得起来")
        }
    })
    .await
    .expect("没 panic");
    // 核心也没有 key：这一轮说「没有可用的模型」。这台机器上的沙盒用不了的（例如 Windows 上 5-9 以前），前面还有
    // 沙盒用不了那一句，照这台机器的样子另有测试（施工 5-4 下）。
    assert_eq!(output.status.code(), Some(5), "{output:?}");
    assert_eq!(
        without_the_sandbox_line(&String::from_utf8_lossy(&output.stderr)),
        "没有可用的模型：设环境变量 DEEPSEEK_API_KEY\n"
    );
    drop(held);
    home.until_stopped().await;
}

#[test]
fn wrong_arguments_are_exit_code_2() {
    let home = Home::new();
    for args in [&[][..], &["--session", "x", "--continue", "在吗"][..]] {
        let output = ask(&home, "C", args);
        assert_eq!(output.status.code(), Some(2), "{args:?}：{output:?}");
    }
}

#[test]
fn the_help_is_the_page_in_the_language() {
    // `miyu ask -h` 印自己写的那一页，一个字节不差（施工 4-11）。
    let home = Home::new();
    for (lang, language) in [("zh_CN.UTF-8", Language::Chinese), ("C", Language::English)] {
        for args in [&["-h"][..], &["--help"]] {
            let output = ask(&home, lang, args);
            assert!(output.status.success(), "{args:?}：{output:?}");
            assert_eq!(
                String::from_utf8_lossy(&output.stdout),
                page(language, Page::Ask),
                "{lang} {args:?}"
            );
        }
    }
}

/// 标准错误去掉最前面沙盒用不了那一句（有的话）：那一句照这台机器能不能用沙盒，别的测试守着。
fn without_the_sandbox_line(stderr: &str) -> &str {
    match stderr.strip_prefix("· 沙盒用不了（") {
        Some(rest) => rest.split_once('\n').map_or("", |(_, after)| after),
        None => stderr,
    }
}
