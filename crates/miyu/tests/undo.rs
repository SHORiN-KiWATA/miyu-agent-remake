//! 真跑 `miyu undo`、`miyu redo`（`docs/construction/4-7-miyu undo、miyu redo（下）.md`）：说明跟着界面语言；核心在跑的，
//! 撤掉上一次 `miyu ask` 的那一轮、再恢复它，两条命令各接对了自己的那一个。

mod support;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

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
async fn run(root: &Path, args: &'static [&'static str]) -> Output {
    let root: PathBuf = root.to_path_buf();
    tokio::task::spawn_blocking(move || miyu(&root, "zh_CN.UTF-8", args))
        .await
        .expect("没 panic")
}

#[test]
fn the_help_follows_the_language() {
    let home = Home::new();
    let undo = miyu(home.root.path(), "zh_CN.UTF-8", &["undo", "--help"]);
    assert!(undo.status.success(), "{undo:?}");
    let undo = String::from_utf8_lossy(&undo.stdout);
    assert!(
        undo.contains("撤掉当前会话的最后一轮，把她改过的文件改回去"),
        "{undo}"
    );
    assert!(
        undo.contains("哪个会话；不写的是上一次 miyu ask 开的那个"),
        "{undo}"
    );
    let redo = miyu(home.root.path(), "zh_CN.UTF-8", &["redo", "--help"]);
    let redo = String::from_utf8_lossy(&redo.stdout);
    assert!(redo.contains("发下一句之前，恢复最近一次撤销"), "{redo}");
    let english = miyu(home.root.path(), "C", &["undo", "--help"]);
    let english = String::from_utf8_lossy(&english.stdout);
    assert!(
        english.contains("Undo the last turn of the current session"),
        "{english}"
    );
}

#[tokio::test]
async fn undo_and_redo_the_last_ask() {
    let home = Home::new();
    let (held, _) = within("拉起", connect_or_start(&home.root, || home.core()))
        .await
        .expect("拉得起");
    let root = home.root.path().to_path_buf();
    // 核心没有 key：这一轮说「没有可用的模型」，可也是一轮。
    let asked = run(&root, &["ask", "在吗"]).await;
    assert_eq!(asked.status.code(), Some(5), "{asked:?}");
    let undone = run(&root, &["undo"]).await;
    assert_eq!(undone.status.code(), Some(0), "{undone:?}");
    assert_eq!(
        String::from_utf8_lossy(&undone.stdout),
        "· 撤销「在吗」这一轮\n发下一句之前，可以用 miyu redo 恢复。\n"
    );
    let redone = run(&root, &["redo"]).await;
    assert_eq!(redone.status.code(), Some(0), "{redone:?}");
    assert_eq!(
        String::from_utf8_lossy(&redone.stdout),
        "· 恢复「在吗」这一轮\n"
    );
    drop(held);
    home.until_stopped().await;
}
