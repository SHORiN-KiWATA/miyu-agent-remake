//! 真跑 `miyu memory`（施工 R-3 再补，`docs/blueprint/cli/memory.md`）：`memory -h`、各个子命令的 `-h`、`help memory` 印的都是
//! 那一页，跟着界面语言；参数写错的退出码 2：`--persona` 和 `-s` 一起写、`--class` 写错、少了话、少了编号、`clear` 没写清哪一种。

use std::path::Path;
use std::process::{Command, Output};

use crate::support::{Home, MIYU};
use miyu_cli::help::{Page, page};
use miyu_cli::language::Language;

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

#[test]
fn every_help_is_the_page_in_the_language() {
    let home = Home::new();
    for (lang, language) in [("zh_CN.UTF-8", Language::Chinese), ("C", Language::English)] {
        for args in [
            &["memory", "-h"][..],
            &["memory", "--help"],
            &["help", "memory"],
            &["memory", "list", "-h"],
            &["memory", "add", "--help"],
            &["memory", "clear", "-h"],
            &["memory", "clear", "me", "-h"],
        ] {
            let output = miyu(home.root.path(), lang, args);
            assert!(output.status.success(), "{args:?}：{output:?}");
            assert_eq!(
                String::from_utf8_lossy(&output.stdout),
                page(language, Page::Memory),
                "{lang} {args:?}"
            );
        }
    }
}

#[test]
fn wrong_arguments_are_exit_code_2() {
    let home = Home::new();
    for args in [
        &["memory", "list", "--persona", "engineer", "-s", "x"][..],
        &["memory", "add", "--class", "nope", "用户养猫"],
        &["memory", "add"],
        &["memory", "edit", "m1"],
        &["memory", "forget"],
        &["memory", "clear"],
        &["memory", "clear", "everything"],
        &["memory", "drop"],
    ] {
        let output = miyu(home.root.path(), "C", args);
        assert_eq!(output.status.code(), Some(2), "{args:?}：{output:?}");
    }
}
