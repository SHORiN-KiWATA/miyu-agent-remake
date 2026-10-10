//! 真跑 `miyu pkg`（施工 T-3，`docs/blueprint/cli/pkg.md`）：列出、照相对路径装一份清单、卸掉；卸掉出厂的标「已卸载」、照编号
//! 装回来；必需的卸不掉、写错的清单装不上，退出码 1；`--format json` 原样印那一串；`-h` 印那一页。看一个包的信息、装了哪些
//! 文件、文件归哪个包、文件改没改，pacman 的写法也认（施工 F-8 下）。

use serde_json::Value;

use crate::support::cli::{run, stderr, stdout};
use crate::support::{Home, within};
use miyu_cli::help::{Page, page};
use miyu_cli::language::Language;
use miyu_ipc::connect_or_start;

/// 一份手动拉起的扩展包的清单（不会真的起）。
const XPKG: &str = "[package]\nprotocol = [1, 1]\nname = { en = \"X\", zh = \"测试包\" }\n\n[command]\nname = \"xpkg\"\nprogram = \"miyu-nothing\"\nabout = { en = \"X\" }\n\n[process]\nstart = \"manual\"\n";

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
    for (id, text) in [("xpkg", XPKG), ("bad", "[package]\n\n[process]\n")] {
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
async fn pkg_tells_about_installed_files_through_a_real_core() {
    let home = Home::new();
    let root = home.root.path().to_path_buf();
    let (held, _) = within("拉起", connect_or_start(&home.root, || home.core()))
        .await
        .expect("拉得起");
    let cwd = home.dir.with_extension("query");
    std::fs::create_dir_all(cwd.join("xpkg/bin")).expect("建得了目录");
    std::fs::write(cwd.join("xpkg/package.toml"), XPKG).expect("写得进");
    std::fs::write(cwd.join("xpkg/bin/data.txt"), "1").expect("写得进");
    let zh = "zh_CN.UTF-8";

    let installed = run(&root, &cwd, zh, &["pkg", "-U", "./xpkg"]).await;
    assert_eq!(
        stdout(&installed),
        "装好了：xpkg\n",
        "pacman 的 -U：{installed:?}"
    );
    let info = stdout(&run(&root, &cwd, zh, &["pkg", "info", "xpkg"]).await);
    assert!(info.starts_with("名称      测试包\n"), "{info}");
    assert!(
        info.contains("个文件") && info.contains("安装时间  "),
        "{info}"
    );

    let files = stdout(&run(&root, &cwd, zh, &["pkg", "-Ql", "xpkg"]).await);
    let data = files
        .lines()
        .find_map(|line| line.strip_prefix("xpkg "))
        .filter(|path| path.ends_with("data.txt"))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| panic!("一个文件一行，绝对路径：{files}"));
    assert!(data.is_absolute() && data.exists(), "{files}");
    let owned = run(&root, &cwd, zh, &["pkg", "-Qo", &data.to_string_lossy()]).await;
    assert_eq!(stdout(&owned), format!("{} 属于 xpkg\n", data.display()));
    let nobody = run(&root, &cwd, zh, &["pkg", "owns", "nothing.txt"]).await;
    assert_eq!(nobody.status.code(), Some(1));
    assert!(
        stderr(&nobody).starts_with("没有软件包包含 "),
        "{}",
        stderr(&nobody)
    );

    let fine = run(&root, &cwd, zh, &["pkg", "-Qk"]).await;
    assert_eq!(
        (fine.status.code(), stdout(&fine)),
        (Some(0), "xpkg：正常\n".to_string())
    );
    std::fs::write(&data, "2").expect("写得进");
    let changed = run(&root, &cwd, zh, &["pkg", "check", "xpkg"]).await;
    assert_eq!(
        (changed.status.code(), stdout(&changed)),
        (Some(1), "xpkg：已修改 bin/data.txt\n".to_string())
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
            &["pkg", "info", "-h"],
            &["pkg", "-Qk", "-h"],
            &["help", "pkg"],
        ] {
            let printed = run(home.root.path(), &cwd, lang, args).await;
            assert_eq!(stdout(&printed), page(language, Page::Pkg), "{args:?}");
        }
    }
}
