//! 真跑 `miyu recap`（施工 3-8 四补，`docs/blueprint/cli/recap.md`）：说明跟着界面语言；核心在跑的，回顾上一次 `miyu ask`
//! 开的那个会话（没有 key 的核心那一轮没回复，没有可回顾的），`-s` 和 `--session` 回顾的是写的那个；没有 key、核心也没在跑
//! 的，不拉起，照 `miyu ask` 说没有可用的模型，退出码 5。

mod support;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use miyu_cli::help::{Page, page};
use miyu_cli::language::Language;
use miyu_ipc::connect_or_start;
use support::{Home, MIYU, within};

/// 在数据根 `root` 上跑 `miyu <args>`：没有 key，界面语言是 `lang`。
fn miyu(root: &Path, lang: &str, args: &[&str]) -> Output {
    Command::new(MIYU)
        .args(args)
        .env("MIYU_HOME", root)
        .env("MIYU_RESOURCES", support::resources())
        .env("LANG", lang)
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .env_remove("DEEPSEEK_API_KEY")
        .env_remove("XDG_RUNTIME_DIR")
        .output()
        .expect("跑得起来")
}

/// 在阻塞线程里跑：核心在这个测试的运行时里。
async fn run(root: &Path, args: Vec<String>) -> Output {
    let root: PathBuf = root.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        miyu(&root, "zh_CN.UTF-8", &args)
    })
    .await
    .expect("没 panic")
}

#[test]
fn the_help_is_the_page_in_the_language() {
    let home = Home::new();
    for (lang, language) in [("zh_CN.UTF-8", Language::Chinese), ("C", Language::English)] {
        for args in [
            &["recap", "-h"][..],
            &["recap", "--help"],
            &["help", "recap"],
        ] {
            let output = miyu(home.root.path(), lang, args);
            assert!(output.status.success(), "{args:?}：{output:?}");
            assert_eq!(
                String::from_utf8_lossy(&output.stdout),
                page(language, Page::Recap),
                "{lang} {args:?}"
            );
        }
    }
}

#[tokio::test]
async fn the_last_ask_or_the_given_session_is_the_one_recapped() {
    let home = Home::new();
    let (held, _) = within("拉起", connect_or_start(&home.root, || home.core()))
        .await
        .expect("拉得起");
    let root = home.root.path().to_path_buf();
    // 核心没有 key：这一轮说「没有可用的模型」，她一句都没回，没有可回顾的。
    let asked = run(&root, vec!["ask".into(), "在吗".into()]).await;
    assert_eq!(asked.status.code(), Some(5), "{asked:?}");
    let recapped = run(&root, vec!["recap".into()]).await;
    assert_eq!(recapped.status.code(), Some(1), "{recapped:?}");
    assert!(recapped.stdout.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&recapped.stderr),
        "还没有可回顾的内容\n"
    );
    // `-s`、`--session` 回顾的是写的那个：写一个不在的，照核心说的。
    let missing = "0192f3a0-1111-7abc-8def-001122334455";
    for flag in ["-s", "--session"] {
        let recapped = run(&root, vec!["recap".into(), flag.into(), missing.into()]).await;
        assert_eq!(recapped.status.code(), Some(1), "{flag}：{recapped:?}");
        assert_eq!(
            String::from_utf8_lossy(&recapped.stderr),
            "没有这个会话。\n",
            "{flag}"
        );
    }
    drop(held);
    home.until_stopped().await;
}

#[test]
fn without_a_key_and_a_core_nothing_is_started() {
    let home = Home::new();
    let recapped = miyu(home.root.path(), "zh_CN.UTF-8", &["recap"]);
    assert_eq!(recapped.status.code(), Some(5), "{recapped:?}");
    assert!(recapped.stdout.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&recapped.stderr),
        "没有可用的模型：设环境变量 DEEPSEEK_API_KEY\n"
    );
    let recapped = miyu(home.root.path(), "C", &["recap"]);
    assert_eq!(recapped.status.code(), Some(5), "{recapped:?}");
    assert_eq!(
        String::from_utf8_lossy(&recapped.stderr),
        "No model available: set DEEPSEEK_API_KEY\n"
    );
    assert!(!home.root.run().join("socket").exists(), "没拉起核心");
    assert!(home.core_log().is_empty());
}
