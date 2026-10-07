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
    // 测试里的核心没有沙盒助手：命令不在沙盒里跑，核心 D-4 的 `detail.sandbox` 是 `false`。
    assert!(screen.contains("沙盒外运行"), "{screen}");
    assert!(
        !screen.contains("这个会话都允许"),
        "跑命令的没有放行规则：{screen}"
    );
    tui.key(b"\r");
    tui.wait_for("看完了。");
}

#[test]
fn ask_user_opens_the_question_drawer_and_the_answers_reach_her() {
    // 核心 D-2：她调 ask_user，核心推 `question.asked`；抽屉里答了经 `session.answer` 交回去，结果写进正文。
    let args = serde_json::json!({"questions": [
        {"header": "语言", "question": "用哪种语言写？", "options": [
            {"label": "Rust（推荐）", "description": "和仓库一样", "preview": "fn main() {}"},
            {"label": "Python"}]},
        {"header": "测试", "question": "要不要写测试？", "options": [{"label": "要"}, {"label": "不要"}]}
    ]})
    .to_string();
    let script = Script::new([
        Play::Calls(vec![("ask_user".into(), args)]),
        Play::Says("好，照你说的写。"),
    ]);
    let home = Home::with_tools(script, "");
    let mut tui = home.tui("zh_CN.UTF-8");
    tui.wait_for("工作区");
    tui.say("帮我写个小工具");
    tui.wait_for("用哪种语言写？");
    tui.wait_for("fn main() {}");
    // 第一道选第一项（Enter），跳到第二道；按 n 补一句，再选「不要」（数字 2），到「确认」页交。
    tui.key(b"\r");
    tui.wait_for("要不要写测试？");
    tui.key(b"n");
    tui.type_text("以后再说");
    tui.key(b"\r");
    tui.key(b"2");
    tui.wait_for("测试：不要");
    tui.key(b"\r");
    tui.wait_for("好，照你说的写。");
    let screen = tui.lines().join("\n");
    assert!(screen.contains("已回答"), "{screen}");
    assert!(screen.contains("语言：Rust（推荐）"), "{screen}");
    assert!(screen.contains("测试：不要（补充：以后再说）"), "{screen}");
}

#[test]
fn asking_to_run_outside_the_sandbox_shows_the_line_and_allow_once_runs_it() {
    // 核心 D-4：她带 `outside_sandbox` 请求在沙盒外跑一条，弹确认，允许这一次照办。
    let args = serde_json::json!({
        "command": "echo outside", "description": "在沙盒外试一下", "outside_sandbox": true
    })
    .to_string();
    let script = Script::new([
        Play::Calls(vec![("shell".into(), args)]),
        Play::Says("跑完了。"),
    ]);
    let home = Home::with_tools(script, "");
    let mut tui = home.tui("zh_CN.UTF-8");
    tui.wait_for("工作区");
    tui.say("试试");
    tui.wait_for("在沙盒外试一下");
    let screen = tui.lines().join("\n");
    assert!(screen.contains("$ echo outside"), "{screen}");
    assert!(screen.contains("沙盒外运行"), "{screen}");
    tui.key(b"\r");
    tui.wait_for("跑完了。");
}

#[test]
fn the_same_call_id_in_a_new_session_still_asks() {
    // 调用编号只在一个会话里唯一：前一个会话了结过的 `call_…`，`/new` 以后新会话又问同一个编号，照样开抽屉
    // （2026-10-07 项目主人报：接上旧会话、`/new` 以后让她在沙盒外跑一条 echo，没弹确认，看着卡住了）。
    let outside = outside("same-call");
    let first = outside.join("one.txt");
    let second = outside.join("two.txt");
    let write = |path: &std::path::Path| {
        let args = serde_json::json!({"file_path": path, "content": "hi"}).to_string();
        Play::Calls(vec![("write".into(), args)])
    };
    let script = Script::new([
        write(&first),
        Play::Says("写好了。"),
        write(&second),
        Play::Says("又写好了。"),
    ]);
    let home = Home::with_tools(script, "");
    let mut tui = home.tui("zh_CN.UTF-8");
    tui.wait_for("工作区");
    tui.say("写个文件");
    tui.wait_for("允许这一次");
    tui.key(b"\r");
    tui.wait_for("写好了。");
    tui.say("/new");
    tui.pump(Duration::from_millis(300));
    tui.say("写个文件");
    tui.wait_for("允许这一次");
    tui.key(b"\r");
    wait(&mut tui, "第二个文件写出来", || second.exists());
    tui.wait_for("又写好了。");
    std::fs::remove_dir_all(&outside).expect("删得掉测试建的目录");
}
