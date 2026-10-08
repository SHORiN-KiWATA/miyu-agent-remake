//! 真跑 `miyu rename`（施工 3-8 五补，`docs/blueprint/cli/rename.md`）：说明跟着界面语言；核心在跑的，给上一次 `miyu ask`
//! 开的那个会话起名，什么都不印、退出码 0，几个词用空格连起来；`-s` 和 `--session` 起名的是写的那个；没写标题的是参数不对。

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use crate::support::{Home, MIYU, within};
use miyu_cli::help::{Page, page};
use miyu_cli::language::Language;
use miyu_ipc::connect_or_start;

/// 在数据根 `root` 上跑 `miyu <args>`：界面语言是 `lang`。
fn miyu(root: &Path, lang: &str, args: &[&str]) -> Output {
    Command::new(MIYU)
        .args(args)
        .env("MIYU_HOME", root)
        .envs(crate::support::offline(root))
        .env("MIYU_RESOURCES", crate::support::resources())
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
fn the_help_is_the_page_in_the_language() {
    let home = Home::new();
    for (lang, language) in [("zh_CN.UTF-8", Language::Chinese), ("C", Language::English)] {
        for args in [
            &["rename", "-h"][..],
            &["rename", "--help"],
            &["help", "rename"],
        ] {
            let output = miyu(home.root.path(), lang, args);
            assert!(output.status.success(), "{args:?}：{output:?}");
            assert_eq!(
                String::from_utf8_lossy(&output.stdout),
                page(language, Page::Rename),
                "{lang} {args:?}"
            );
        }
    }
    // 没写标题：参数不对，退出码 2。
    let output = miyu(home.root.path(), "C", &["rename"]);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
}

#[tokio::test]
async fn the_last_ask_or_the_given_session_is_the_one_renamed() {
    let home = Home::new();
    home.system_config(crate::support::UNUSABLE_MODEL);
    let (held, _) = within("拉起", connect_or_start(&home.root, || home.core()))
        .await
        .expect("拉得起");
    let root = home.root.path().to_path_buf();
    // 配的模型用不了（驱动还没有，施工 8-11 起没配的 `miyu ask` 不造会话）：这一轮说「没有可用的模型」，会话照样开了。
    let asked = run(&root, vec!["ask".into(), "在吗".into()]).await;
    assert_eq!(asked.status.code(), Some(5), "{asked:?}");
    let renamed = run(&root, vec!["rename".into(), "问".into(), "在不在".into()]).await;
    assert_eq!(renamed.status.code(), Some(0), "{renamed:?}");
    assert!(
        renamed.stdout.is_empty() && renamed.stderr.is_empty(),
        "{renamed:?}"
    );
    let sessions = home.root.path().join("home/admin/sessions");
    let session = std::fs::read_dir(&sessions)
        .expect("有会话目录")
        .next()
        .expect("开了一个会话")
        .expect("读得到")
        .path();
    let log = std::fs::read_dir(&session)
        .expect("会话目录读得到")
        .map(|entry| std::fs::read_to_string(entry.expect("读得到").path()).unwrap_or_default())
        .collect::<String>();
    assert!(
        log.contains(r#""kind":"session.meta_changed""#) && log.contains(r#""title":"问 在不在""#),
        "{log}"
    );
    // `-s`、`--session` 起名的是写的那个：写一个不在的，照核心说的。
    let missing = "0192f3a0-1111-7abc-8def-001122334455";
    for flag in ["-s", "--session"] {
        let renamed = run(
            &root,
            vec!["rename".into(), flag.into(), missing.into(), "名".into()],
        )
        .await;
        assert_eq!(renamed.status.code(), Some(1), "{flag}：{renamed:?}");
        assert_eq!(
            String::from_utf8_lossy(&renamed.stderr),
            "没有这个会话。\n",
            "{flag}"
        );
    }
    drop(held);
    home.until_stopped().await;
}
