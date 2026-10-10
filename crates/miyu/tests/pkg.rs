//! 真跑 `miyu pkg`（施工 T-3，`docs/blueprint/cli/pkg.md`）：列出、照相对路径装一份清单、卸掉；卸掉出厂的标「已卸载」、照编号
//! 装回来；必需的卸不掉、写错的清单装不上，退出码 1；`--format json` 原样印那一串；`-h` 印那一页。

use serde_json::Value;

use crate::support::cli::{run, stderr, stdout};
use crate::support::{Home, within};
use miyu_cli::help::{Page, page};
use miyu_cli::language::Language;
use miyu_ipc::connect_or_start;

/// 一份手动拉起的扩展包的清单（不会真的起）。
const XPKG: &str = "[package]\nkind = \"process\"\nprotocol = [1, 1]\nname = { en = \"X\", zh = \"测试包\" }\n\n[command]\nname = \"xpkg\"\nprogram = \"miyu-nothing\"\nabout = { en = \"X\" }\n\n[process]\nstart = \"manual\"\n";

/// 列出来的那几行里，编号是 `id` 的那一行。
fn line<'a>(listed: &'a str, id: &str) -> Option<&'a str> {
    listed
        .lines()
        .find(|line| line.split_whitespace().next() == Some(id))
}

#[tokio::test]
async fn pkg_lists_installs_and_removes_through_a_real_core() {
    let home = Home::new();
    let root = home.root.path().to_path_buf();
    let (held, _) = within("拉起", connect_or_start(&home.root, || home.core()))
        .await
        .expect("拉得起");
    let cwd = home.dir.with_extension("work");
    std::fs::create_dir_all(&cwd).expect("建得了目录");
    for (id, text) in [("xpkg", XPKG), ("bad", "[package]\nkind = \"process\"\n")] {
        std::fs::create_dir_all(cwd.join(id)).expect("建得了目录");
        std::fs::write(cwd.join(id).join("package.toml"), text).expect("写得进");
    }
    let zh = "zh_CN.UTF-8";

    let listed = run(&root, &cwd, zh, &["pkg"]).await;
    assert_eq!(listed.status.code(), Some(0), "{listed:?}");
    let text = stdout(&listed);
    assert!(
        line(&text, "basesystem").is_some_and(|line| line.ends_with("基础系统")),
        "{text}"
    );

    let installed = run(&root, &cwd, zh, &["pkg", "install", "./xpkg"]).await;
    assert_eq!(stdout(&installed), "装好了：xpkg\n", "{installed:?}");
    let text = stdout(&run(&root, &cwd, zh, &["pkg", "list"]).await);
    assert!(
        line(&text, "xpkg").is_some_and(|line| line.ends_with("测试包")),
        "{text}"
    );
    let removed = run(&root, &cwd, zh, &["pkg", "remove", "xpkg"]).await;
    assert_eq!(stdout(&removed), "卸掉了：xpkg\n", "{removed:?}");
    assert!(line(&stdout(&run(&root, &cwd, zh, &["pkg"]).await), "xpkg").is_none());

    // 出厂的：卸掉的照样列、标「已卸载」，照编号装回来。
    run(&root, &cwd, zh, &["pkg", "remove", "net"]).await;
    let text = stdout(&run(&root, &cwd, zh, &["pkg"]).await);
    assert!(
        line(&text, "net").is_some_and(|line| line.ends_with("（已卸载）")),
        "{text}"
    );
    let back = run(&root, &cwd, zh, &["pkg", "install", "net"]).await;
    assert_eq!(stdout(&back), "装好了：net\n", "{back:?}");

    let required = run(&root, &cwd, zh, &["pkg", "remove", "basesystem"]).await;
    assert_eq!(required.status.code(), Some(1));
    assert_eq!(stderr(&required), "必需的软件包，无法卸载。\n");
    let bad = run(&root, &cwd, zh, &["pkg", "install", "bad/"]).await;
    assert_eq!(bad.status.code(), Some(1));
    assert!(stderr(&bad).starts_with("装不上："), "{}", stderr(&bad));

    let json = run(&root, &cwd, zh, &["pkg", "--format", "json"]).await;
    let packages: Value = serde_json::from_str(&stdout(&json)).expect("是 JSON");
    assert!(
        packages
            .as_array()
            .is_some_and(|all| all.iter().any(|one| one["package"] == "basesystem")),
        "{packages}"
    );
    drop(held);
    if let Err(error) = std::fs::remove_dir_all(&cwd) {
        eprintln!("临时目录没删掉：{error}");
    }
}

#[tokio::test]
async fn every_help_is_the_page_in_the_language() {
    let home = Home::new();
    let cwd = std::env::temp_dir();
    for (lang, language) in [("zh_CN.UTF-8", Language::Chinese), ("C", Language::English)] {
        for args in [
            &["pkg", "-h"][..],
            &["pkg", "install", "-h"],
            &["pkg", "remove", "-h"],
            &["help", "pkg"],
        ] {
            let printed = run(home.root.path(), &cwd, lang, args).await;
            assert_eq!(stdout(&printed), page(language, Page::Pkg), "{args:?}");
        }
    }
}
