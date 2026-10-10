//! 真跑 `miyu compact`（施工 6-8，`docs/blueprint/cli/compact.md`）：说明跟着界面语言；核心在跑的，压上一次 `miyu ask`
//! 开的那个会话（说得短的没有能压的），`-s` 和 `--session` 压的是写的那个；核心没配模型的，照 `miyu ask` 说没有可用的模型，退出码 5。

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
            &["compact", "-h"][..],
            &["compact", "--help"],
            &["help", "compact"],
        ] {
            let output = miyu(home.root.path(), lang, args);
            assert!(output.status.success(), "{args:?}：{output:?}");
            assert_eq!(
                String::from_utf8_lossy(&output.stdout),
                page(language, Page::Compact),
                "{lang} {args:?}"
            );
        }
    }
}

#[tokio::test]
async fn the_last_ask_or_the_given_session_is_the_one_asked() {
    let home = Home::new();
    home.system_config(crate::support::UNUSABLE_MODEL);
    let (held, _) = within("拉起", connect_or_start(&home.root, || home.core()))
        .await
        .expect("拉得起");
    let root = home.root.path().to_path_buf();
    // 配的模型用不了（驱动还没有，施工 8-11 起没配的 `miyu ask` 不造会话）：这一轮说「没有可用的模型」，可也说了一句。说得短，全在尾巴里，没有能压的。
    let asked = run(&root, vec!["ask".into(), "在吗".into()]).await;
    assert_eq!(asked.status.code(), Some(5), "{asked:?}");
    let refused = "没有可压缩的内容。\n";
    let compacted = run(&root, vec!["compact".into(), "重点".into(), "保留".into()]).await;
    assert_eq!(compacted.status.code(), Some(1), "{compacted:?}");
    assert!(compacted.stdout.is_empty());
    assert_eq!(String::from_utf8_lossy(&compacted.stderr), refused);
    // `-s`、`--session` 压的是写的那个：写一个不在的，照核心说的。
    let missing = "0192f3a0-1111-7abc-8def-001122334455";
    for flag in ["-s", "--session"] {
        let compacted = run(&root, vec!["compact".into(), flag.into(), missing.into()]).await;
        assert_eq!(compacted.status.code(), Some(1), "{flag}：{compacted:?}");
        assert_eq!(
            String::from_utf8_lossy(&compacted.stderr),
            "会话不存在。\n",
            "{flag}"
        );
    }
    drop(held);
    home.until_stopped().await;
}
