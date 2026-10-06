//! 伪终端里的端到端测试：确认的抽屉接真的核心（蓝图 `tui.md`「确认和提问的抽屉」，核心 D-1）。她往工作区外写文件，
//! 核心来问；抽屉里答了经 `session.answer` 交回去，核心照办。
//!
//! 只在 Unix 上跑：工作区外的目录用 `/var/tmp`（临时目录核心本来就让写），命令用 `uname`；Windows 上要另找一处
//! 不在临时目录、又不碰人的家目录的地方，那时再加。
#![cfg(unix)]

mod support;

use std::time::{Duration, Instant};

use miyu_session::testkit::{Play, Script};
use support::Home;

const DOWN: &[u8] = b"\x1b[B";

/// 工作区外的一个目录：临时目录核心本来就让写，用 `/var/tmp`（真实路径，macOS 上它是链接），测完删掉。
fn outside(name: &str) -> std::path::PathBuf {
    let dir =
        std::path::Path::new("/var/tmp").join(format!("miyu-tui-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("建得了");
    std::fs::canonicalize(&dir).expect("在")
}

/// 等到条件成立。
fn wait(tui: &mut support::Tui, what: &str, done: impl Fn() -> bool) {
    let end = Instant::now() + support::WAIT;
    while !done() {
        assert!(
            Instant::now() < end,
            "等不到：{what}\n{}",
            tui.lines().join("\n")
        );
        tui.pump(Duration::from_millis(100));
    }
}

#[test]
fn a_write_outside_the_workspace_asks_and_allowing_once_lets_it_through() {
    let outside = outside("asking");
    let target = outside.join("note.txt");
    let args = serde_json::json!({"file_path": target, "content": "hi"}).to_string();
    let script = Script::new([
        Play::Calls(vec![("write".into(), args)]),
        Play::Says("写好了。"),
    ]);
    let home = Home::with_tools(script, "");
    let mut tui = home.tui("zh_CN.UTF-8");
    tui.wait_for("工作区");
    tui.say("写个文件");
    tui.wait_for("允许这一次");
    assert!(!target.exists(), "还没答就没写");
    // 有放行规则：三项都在。
    tui.wait_for("这个会话都允许");
    assert!(
        !tui.lines().iter().any(|l| l.contains("在问")),
        "她自己问的不写谁在问"
    );
    tui.key(b"\r");
    wait(&mut tui, "文件写出来", || target.exists());
    tui.wait_for("写好了。");
    assert!(
        !tui.lines().iter().any(|l| l.contains("允许这一次")),
        "答了收起"
    );
    std::fs::remove_dir_all(&outside).expect("删得掉测试建的目录");
}

#[test]
fn denying_with_a_reason_tells_her_and_writes_a_red_line() {
    let outside = outside("deny");
    let target = outside.join("note.txt");
    let args = serde_json::json!({"file_path": target, "content": "hi"}).to_string();
    let script = Script::new([
        Play::Calls(vec![("write".into(), args)]),
        Play::Says("好，不写了。"),
    ]);
    let home = Home::with_tools(script, "");
    let mut tui = home.tui("zh_CN.UTF-8");
    tui.wait_for("工作区");
    tui.say("写个文件");
    tui.wait_for("允许这一次");
    // 移到「不允许」就在写理由：直接打字，回车交（2026-10-07 项目主人：不用按两次回车）。
    tui.key(DOWN);
    tui.key(DOWN);
    tui.type_text("别写这里");
    tui.key(b"\r");
    tui.wait_for("不允许 · 别写这里");
    tui.wait_for("好，不写了。");
    assert!(!target.exists(), "拒了没写");
    std::fs::remove_dir_all(&outside).expect("删得掉测试建的目录");
}

#[test]
fn a_command_approval_shows_its_short_title_and_the_command() {
    // 2026-10-07 项目主人：确认时看不到具体的命令；问题行写成了「要用 shell」，应该是这条命令的短标题。
    let args = r#"{"command": "uname -a", "description": "看看系统信息"}"#;
    let script = Script::new([
        Play::Calls(vec![("shell".into(), args.into())]),
        Play::Says("看完了。"),
    ]);
    let home = Home::with_tools(script, "");
    let mut tui = home.tui("zh_CN.UTF-8");
    tui.wait_for("工作区");
    tui.say("看看系统");
    tui.wait_for("允许这一次");
    let screen = tui.lines().join("\n");
    assert!(
        screen.contains("看看系统信息"),
        "短标题当问题行：\n{screen}"
    );
    assert!(screen.contains("$ uname -a"), "命令原文：\n{screen}");
    assert!(!screen.contains("要用 shell"), "{screen}");
    tui.key(b"\r");
    tui.wait_for("看完了。");
}
