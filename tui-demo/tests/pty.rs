//! 伪终端里的端到端测试（蓝图 `tui.md`「守着它的」）：真界面连一份照剧本回话的核心，读屏幕、发按键。
//! 这几样原来只在临时目录里的 pyte 测具上接真模型测过，搬进仓库以后 `cargo test` 就跑得到。

mod support;

use std::time::Duration;

use miyu_session::testkit::{Play, Script};
use support::Home;

const UP: &[u8] = b"\x1b[A";
const DOWN: &[u8] = b"\x1b[B";
const ESC: &[u8] = b"\x1b";

#[test]
fn a_scripted_reply_is_drawn_with_its_done_line() {
    let home = Home::new(Script::new([Play::Says("你好，我在。")]));
    let mut tui = home.tui("zh_CN.UTF-8");
    tui.wait_for("工作区");
    tui.say("在吗");
    tui.wait_for("你好，我在。");
    tui.wait_for("▣  ");
    assert!(tui.shows("┃ 在吗"), "{}", tui.lines().join("\n"));
}

#[test]
fn recap_says_there_is_nothing_yet_then_draws_a_block() {
    // 2026-10-01 回顾（`/recap`）：还没开会话时只提示；她答过以后要一段，画成暗色的「回顾：」。
    let home = Home::new(Script::new([
        Play::Says("你好。"),
        Play::Says("在打招呼，没有别的事。"),
    ]));
    let mut tui = home.tui("zh_CN.UTF-8");
    tui.wait_for("工作区");
    tui.say("/recap");
    tui.wait_for("还没有可回顾的内容");
    tui.say("在吗");
    tui.wait_for("▣  ");
    tui.say("/recap");
    tui.wait_for("※ 回顾：在打招呼，没有别的事。");
}

#[test]
fn an_undone_turn_takes_its_recap_along_and_restore_brings_it_back() {
    // 2026-10-01 项目主人报：撤销以后旧的回顾还在，讲的是撤掉了的内容。
    let home = Home::new(Script::new([
        Play::Says("第一轮的回答。"),
        Play::Says("第二轮的回答。"),
        Play::Says("讲到第二轮。"),
    ]));
    let mut tui = home.tui("zh_CN.UTF-8");
    tui.wait_for("工作区");
    tui.say("一");
    tui.wait_for("第一轮的回答。");
    tui.pump(Duration::from_millis(300));
    tui.say("二");
    tui.wait_for("第二轮的回答。");
    tui.pump(Duration::from_millis(300));
    tui.say("/recap");
    tui.wait_for("回顾：讲到第二轮。");
    tui.say("/undo");
    tui.wait_for("已撤销");
    tui.pump(Duration::from_millis(1000));
    assert!(!tui.shows("回顾："), "{}", tui.lines().join("\n"));
    // 撤销把那句整段选中放回输入框（「输入框」）：直接打命令就换掉它。
    tui.say("/restore");
    tui.wait_for("第二轮的回答。");
    tui.wait_for("回顾：讲到第二轮。");
}

#[test]
fn up_and_down_walk_the_history_past_recorded_commands() {
    // 2026-10-01 项目主人报：往上翻得动、往下翻不回来。翻到记着的命令时命令列表弹出来，把 ↑ ↓ 拿去选命令了。
    let home = Home::new(Script::new([]));
    let mut tui = home.tui("zh_CN.UTF-8");
    tui.wait_for("工作区");
    tui.say("/help");
    tui.pump(Duration::from_millis(400));
    tui.key(ESC);
    tui.say("/copy");
    tui.pump(Duration::from_millis(400));
    let mut walk = Vec::new();
    for key in [UP, UP, DOWN, DOWN] {
        tui.key(key);
        walk.push(tui.input());
    }
    assert_eq!(walk[0], "/copy");
    assert_eq!(walk[1], "/help");
    assert_eq!(walk[2], "/copy");
    assert!(
        !walk[3].starts_with('/'),
        "翻回没发的那句（空的，写着提示）：{walk:?}"
    );
}

#[test]
fn a_manually_chosen_language_writes_the_folded_line_in_it() {
    // 2026-10-01 项目主人定：界面语言自动时收起那一行英文，手动选了哪种照哪种。
    let home = Home::new(Script::new([Play::Thinks {
        thinking: "想一想怎么回。",
        text: "好。",
    }]));
    let mut tui = home.tui("zh_CN.UTF-8");
    tui.wait_for("工作区");
    tui.say("在吗");
    tui.wait_for("▣  ");
    tui.wait_for("Thought for");
    tui.say("/language");
    tui.wait_for("自动（跟随系统：中文）");
    tui.key(DOWN);
    tui.key(b"\r");
    tui.wait_for("思考了");
    assert!(!tui.shows("Thought for"), "{}", tui.lines().join("\n"));
}

#[test]
fn rename_sets_removes_and_refuses_a_too_long_title() {
    // 2026-10-01 项目主人定先做 `/rename`（蓝图「改名」）。
    let home = Home::new(Script::new([Play::Says("好。")]));
    let mut tui = home.tui("zh_CN.UTF-8");
    tui.wait_for("工作区");
    tui.say("在吗");
    tui.wait_for("▣  ");
    tui.say("/rename   回文函数  ");
    tui.wait_for("已改名：回文函数");
    tui.pump(Duration::from_millis(500));
    tui.say(&format!("/rename {}", "长".repeat(201)));
    tui.wait_for("标题最多 200 个字");
    tui.pump(Duration::from_millis(500));
    tui.say("/rename");
    tui.wait_for("已去掉标题");
    // `/new` 以后、说第一句以前还没开会话。
    tui.pump(Duration::from_millis(500));
    tui.say("/new");
    tui.pump(Duration::from_millis(500));
    tui.say("/rename 早");
    tui.wait_for("还没开会话，说一句再改名");
}

#[test]
fn the_language_in_personal_settings_wins_over_the_system_one() {
    // 2026-10-01 项目主人定 A：界面语言写进个人设置的 `ui.language`，手动选了就是这个人所有的头都用这种。
    let home = Home::with_settings(Script::new([]), "[ui]\nlanguage = \"en\"\n");
    let mut tui = home.tui("zh_CN.UTF-8");
    tui.wait_for("Workspace");
}

#[test]
fn choosing_a_language_writes_it_into_personal_settings() {
    let home = Home::new(Script::new([]));
    let mut tui = home.tui("zh_CN.UTF-8");
    tui.wait_for("工作区");
    tui.say("/language");
    tui.wait_for("自动（跟随系统：中文）");
    // 第 0 行自动，下面照语言表：中文、English、日本語。
    for _ in 0..3 {
        tui.key(DOWN);
    }
    tui.key(b"\r");
    tui.wait_for("ワークスペース");
    let end = std::time::Instant::now() + support::WAIT;
    while !home.settings().contains("language = \"ja\"") {
        assert!(
            std::time::Instant::now() < end,
            "没写进个人设置：{:?}",
            home.settings()
        );
        tui.pump(Duration::from_millis(100));
    }
    // 换回自动：写 `auto`。
    tui.say("/language");
    tui.pump(Duration::from_millis(400));
    for _ in 0..3 {
        tui.key(b"\x1b[A");
    }
    tui.key(b"\r");
    tui.wait_for("工作区");
    let end = std::time::Instant::now() + support::WAIT;
    while !home.settings().contains("language = \"auto\"") {
        assert!(std::time::Instant::now() < end, "{:?}", home.settings());
        tui.pump(Duration::from_millis(100));
    }
}

#[test]
fn the_at_list_asks_the_core_and_the_data_root_stays_closed() {
    // 2026-10-01 核心 W-2：`@` 列文件、找文件问核心（`fs.list`、`fs.find`），不再自己读目录。数据根里、工作区以外的
    // 核心不列：自己读目录时会列出来，所以这一条认得出问的是不是核心。
    let home = Home::new(Script::new([]));
    std::fs::create_dir_all(home.work.join("src")).unwrap();
    std::fs::write(home.work.join("src/main.rs"), b"fn main() {}").unwrap();
    let mut tui = home.tui("zh_CN.UTF-8");
    tui.wait_for("工作区");
    tui.type_text("@main");
    tui.wait_for("src/main.rs");
    tui.key(ESC);
    tui.key(b"\x03");
    tui.pump(Duration::from_millis(300));
    tui.type_text(&format!("@{}/", home.root().display()));
    tui.wait_for("没有对得上的");
    assert!(!tui.shows("home/"), "{}", tui.lines().join("\n"));
}

#[test]
fn ctrl_j_and_k_move_in_an_open_list() {
    // 2026-10-01 项目主人：能搜的列表里要用 vim 的 Ctrl+J、Ctrl+K 上下。
    let home = Home::new(Script::new([]));
    let mut tui = home.tui("zh_CN.UTF-8");
    tui.wait_for("工作区");
    tui.say("/language");
    tui.wait_for("自动（跟随系统：中文）");
    // 第 0 行自动，下面中文、English、日本語：下、下、上、下、下，停在日本語。
    for key in [b"\x0a", b"\x0a", b"\x0b", b"\x0a", b"\x0a"] {
        tui.key(key);
    }
    tui.key(b"\r");
    tui.wait_for("ワークスペース");
}

#[test]
fn commands_that_change_nothing_only_flash_a_notice() {
    // 2026-10-01 项目主人：没有能撤销的时候不该在正文里打一行，弹通知就行；没有能恢复的、回答进行中、
    // 还没做的命令同一天一起改。通知 `notice_ms`（2 秒）后消失，正文里的行不会，所以等它消失。
    let home = Home::new(Script::new([Play::Says("好。")]));
    let mut tui = home.tui("zh_CN.UTF-8");
    tui.wait_for("工作区");
    tui.say("在吗");
    tui.wait_for("▣  ");
    tui.say("/undo");
    tui.wait_for("已撤销");
    tui.pump(Duration::from_millis(500));
    for (command, words) in [("/undo", "没有能撤销的"), ("/readonly", "是演示用的假命令")]
    {
        tui.say(command);
        tui.wait_for(words);
        tui.pump(Duration::from_millis(2600));
        assert!(!tui.shows(words), "{command}：{}", tui.lines().join("\n"));
    }
    tui.say("/restore");
    tui.wait_for("好。");
    tui.pump(Duration::from_millis(500));
    tui.say("/restore");
    tui.wait_for("没有能恢复的撤销");
    tui.pump(Duration::from_millis(2600));
    assert!(!tui.shows("没有能恢复的撤销"), "{}", tui.lines().join("\n"));
}
