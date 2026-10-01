//! 真跑 `miyu ask`（`docs/construction/3-9-miyu-ask（下）.md`）：核心没配模型的，退出码 5、不造会话（施工 8-6 起 key 来自配置，头
//! 一律拉起核心；施工 8-11 起说话之前先看）；参数不对的退出码 2；帮助页跟着界面语言。

mod support;

use std::process::{Command, Output};

use miyu_cli::help::{Page, page};
use miyu_cli::language::Language;
use miyu_ipc::connect_or_start;
use support::{Home, MIYU, within};

/// 在临时的数据根上跑 `miyu ask <args>`：界面语言是 `lang`。
fn ask(home: &Home, lang: &str, args: &[&str]) -> Output {
    Command::new(MIYU)
        .arg("ask")
        .args(args)
        .env("MIYU_HOME", home.root.path())
        .envs(support::offline(home.root.path()))
        .env("MIYU_RESOURCES", support::resources())
        .env("LANG", lang)
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .env_remove("XDG_RUNTIME_DIR")
        .output()
        .expect("跑得起来")
}

#[tokio::test]
async fn a_core_without_a_model_says_so() {
    let home = Home::new();
    let (held, _) = within("拉起", connect_or_start(&home.root, || home.core()))
        .await
        .expect("拉得起");
    let output = tokio::task::spawn_blocking({
        let home_root = home.root.path().to_path_buf();
        move || {
            Command::new(MIYU)
                .args(["ask", "在吗"])
                .env("MIYU_HOME", &home_root)
                .envs(support::offline(&home_root))
                .env("LANG", "zh_CN.UTF-8")
                .env_remove("LC_ALL")
                .env_remove("LC_MESSAGES")
                .output()
                .expect("跑得起来")
        }
    })
    .await
    .expect("没 panic");
    // 核心也没配模型：这一轮说「没有可用的模型」。这台机器上的沙盒用不了的（例如 Windows 上 5-9 以前），前面还有
    // 沙盒用不了那一句，照这台机器的样子另有测试（施工 5-4 下）。
    assert_eq!(output.status.code(), Some(5), "{output:?}");
    assert_eq!(
        without_the_sandbox_line(&String::from_utf8_lossy(&output.stderr)),
        "没有可用的模型：还没配。运行 miyu setup。\n"
    );
    // 施工 8-11：说话之前先看有没有模型，不在终端里的不造会话（会话目录里一个都没有）。
    let admin = miyu_kernel::id::AccountId::parse("admin").expect("合写法");
    assert_eq!(home.root.sessions(&admin).expect("读得了"), Vec::new());
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
