//! 真跑 `miyu redo`（施工 4-7 再补，`docs/blueprint/cli/redo.md`）：帮助页跟着界面语言，是重做那一页、不是恢复那一页；核心在跑的，
//! 重做上一次 `miyu ask` 的那一轮，先说撤掉了哪一轮，`-s` 和 `--session` 重做的是写的那个；核心没配模型的，照 `miyu ask` 说没有
//! 可用的模型，退出码 5。

mod support;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use miyu_cli::help::{Page, page};
use miyu_cli::language::Language;
use miyu_ipc::connect_or_start;
use support::{Home, MIYU, within};

/// 在数据根 `root` 上跑 `miyu <args>`：界面语言是 `lang`。
fn miyu(root: &Path, lang: &str, args: &[&str]) -> Output {
    Command::new(MIYU)
        .args(args)
        .env("MIYU_HOME", root)
        .env("MIYU_RESOURCES", support::resources())
        .env("LANG", lang)
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
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
fn the_help_is_the_redo_page_in_the_language() {
    let home = Home::new();
    for (lang, language) in [("zh_CN.UTF-8", Language::Chinese), ("C", Language::English)] {
        for args in [&["redo", "-h"][..], &["redo", "--help"], &["help", "redo"]] {
            let output = miyu(home.root.path(), lang, args);
            assert!(output.status.success(), "{args:?}：{output:?}");
            assert_eq!(
                String::from_utf8_lossy(&output.stdout),
                page(language, Page::Redo),
                "{lang} {args:?}"
            );
        }
        assert_ne!(page(language, Page::Redo), page(language, Page::Restore));
    }
}

#[tokio::test]
async fn the_last_ask_or_the_given_session_is_the_one_redone() {
    let home = Home::new();
    let (held, _) = within("拉起", connect_or_start(&home.root, || home.core()))
        .await
        .expect("拉得起");
    let root = home.root.path().to_path_buf();
    // 核心没配模型：这一轮说「没有可用的模型」，可那一句记下了，是人开的一轮，重做得了；新的一轮照样没有模型。
    let asked = run(&root, vec!["ask".into(), "在吗".into()]).await;
    assert_eq!(asked.status.code(), Some(5), "{asked:?}");
    let redone = run(&root, vec!["redo".into()]).await;
    assert_eq!(redone.status.code(), Some(5), "{redone:?}");
    assert!(redone.stdout.is_empty());
    // 先说撤掉了哪一轮（主程序旁边没有沙盒的助手时，照 `miyu ask` 更前面还有沙盒用不了那一句），最后是为什么结束。
    let said = String::from_utf8_lossy(&redone.stderr);
    let lines: Vec<&str> = said.lines().collect();
    let header = lines
        .iter()
        .position(|line| *line == "· 撤销「在吗」这一轮，重新做")
        .unwrap_or_else(|| panic!("{said}"));
    assert!(
        lines[..header]
            .iter()
            .all(|line| line.starts_with("· 沙盒用不了")),
        "{said}"
    );
    assert_eq!(
        lines.last(),
        Some(&"没有可用的模型：还没配。用 miyu config edit --system 写一家供应商和 models.chat。"),
        "{said}"
    );
    // `-s`、`--session` 重做的是写的那个：写一个不在的，照核心说的。
    let missing = "0192f3a0-1111-7abc-8def-001122334455";
    for flag in ["-s", "--session"] {
        let redone = run(&root, vec!["redo".into(), flag.into(), missing.into()]).await;
        assert_eq!(redone.status.code(), Some(1), "{flag}：{redone:?}");
        assert_eq!(
            String::from_utf8_lossy(&redone.stderr),
            "没有这个会话。\n",
            "{flag}"
        );
    }
    drop(held);
    home.until_stopped().await;
}
