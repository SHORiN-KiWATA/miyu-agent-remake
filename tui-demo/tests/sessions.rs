//! 会话列表和切会话（蓝图 `tui.md`「会话列表 `/sessions`」）：伪终端里的真界面连一份照剧本回话的核心。

mod support;

use std::time::Duration;

use miyu_session::testkit::{Play, Script};
use support::{Home, Tui};

const DOWN: &[u8] = b"\x1b[B";
const CTRL_D: &[u8] = b"\x04";
const CTRL_P: &[u8] = b"\x10";

/// 开 `n` 个会话，第 i 个说「第 i 号的话」、答「第 i 号的回答。」，最后停在第 n 个。
fn sessions(n: usize) -> (Home, Tui) {
    let plays: Vec<Play> = (1..=n)
        .map(|i| Play::Says(Box::leak(format!("第 {i} 号的回答。").into_boxed_str())))
        .collect();
    let home = Home::new(Script::new(plays));
    let mut tui = home.tui("zh_CN.UTF-8");
    tui.wait_for("工作区");
    for i in 1..=n {
        if i > 1 {
            tui.say("/new");
            tui.pump(Duration::from_millis(200));
        }
        tui.say(&format!("第 {i} 号的话"));
        tui.wait_for(&format!("第 {i} 号的回答。"));
        tui.pump(Duration::from_millis(200));
    }
    (home, tui)
}

/// 列表里的几行（带短编号的）。
fn rows(tui: &Tui) -> Vec<String> {
    tui.lines()
        .into_iter()
        .filter(|l| l.contains(" #"))
        .collect()
}

#[test]
fn starting_in_the_most_recent_session_draws_what_was_said_before() {
    // 2026-10-01 项目主人要的：启动时进最近的那个会话（第 8 条），以前的对话照补发来的画出来。
    let home = Home::new(Script::new([Play::Thinks {
        thinking: "想一想。",
        text: "以前的回答。",
    }]));
    let mut first = home.tui("zh_CN.UTF-8");
    first.wait_for("工作区");
    first.say("以前说的话");
    first.wait_for("▣  ");
    drop(first);
    let mut again = home.tui_with("zh_CN.UTF-8", &[("MIYU_TUI_START", "recent")]);
    again.wait_for("以前的回答。");
    assert!(again.shows("┃ 以前说的话"), "{}", again.lines().join("\n"));
    again.wait_for("▣  ");
    again.wait_for("Thought for");
}

#[test]
fn switching_draws_the_other_ones_past_without_flashing_an_empty_session() {
    // 2026-10-01 项目主人：切会话会闪一下空会话的首页。补完了才换上来，中间照旧显示原来那个（第 5 条）。
    let (_home, mut tui) = sessions(2);
    tui.say("/sessions");
    tui.wait_for("2 个");
    tui.wait_for("当前");
    tui.key(CTRL_D);
    tui.wait_for("正在用的会话不能删");
    tui.key(DOWN);
    tui.record();
    tui.key(b"\r");
    tui.wait_for("第 1 号的回答。");
    let frames = tui.recorded();
    // 空会话的首页画吉祥物，吉祥物的字里有一串 `@`，正文里没有。
    assert!(
        !frames.windows(3).any(|w| w == b"@@@"),
        "中间闪过空会话的首页"
    );
    assert!(tui.shows("┃ 第 1 号的话"), "{}", tui.lines().join("\n"));
    assert!(!tui.shows("第 2 号的回答。"), "{}", tui.lines().join("\n"));
    // 切回来：列表照造的先后，新的（2 号）在第一行。
    tui.say("/sessions");
    tui.wait_for("2 个");
    tui.key(b"\r");
    tui.wait_for("第 2 号的回答。");
    assert!(!tui.shows("第 1 号的回答。"), "{}", tui.lines().join("\n"));
}

#[test]
fn a_long_list_shows_ten_rows_and_scrolls_with_the_pick() {
    // 2026-10-01 项目主人：会话多了列表铺满整屏。最多露 10 行（第 1 条）。
    let (_home, mut tui) = sessions(12);
    tui.say("/sessions");
    tui.wait_for("12 个");
    assert_eq!(rows(&tui).len(), 10, "{}", tui.lines().join("\n"));
    for _ in 0..11 {
        tui.key(DOWN);
    }
    let shown = rows(&tui);
    assert_eq!(shown.len(), 10);
    assert!(
        shown.last().unwrap().contains('❯'),
        "选到最后一个，露最后十个：{shown:?}"
    );
}

#[test]
fn pinning_moves_it_to_the_top() {
    let (_home, mut tui) = sessions(2);
    tui.say("/sessions");
    tui.wait_for("2 个");
    tui.key(DOWN);
    tui.key(CTRL_P);
    tui.wait_for("已置顶");
    let shown = rows(&tui);
    assert!(!shown[0].contains("当前"), "置顶的旧会话排第一：{shown:?}");
    assert!(shown[1].contains("当前"), "{shown:?}");
}

#[test]
fn ticked_ones_go_on_one_confirmation_and_the_pick_stays_in_place() {
    // 2026-10-01 项目主人：删除不顺、光标跳回第一个、不能批量删。空格勾，Ctrl+D 按两次删勾了的（第 2 条）。
    let (_home, mut tui) = sessions(4);
    tui.say("/sessions");
    tui.wait_for("4 个");
    // 第一行是正在用的 4 号：勾不上。
    tui.key(b" ");
    assert!(!tui.shows("已勾"), "{}", tui.lines().join("\n"));
    tui.key(DOWN);
    tui.key(b" ");
    tui.key(DOWN);
    tui.key(b" ");
    tui.wait_for("已勾 2");
    // Ctrl+A：全都勾着的时候全部取消，没全勾的勾上全部（正在用的除外）。
    tui.key(b"\x01");
    tui.wait_for("已勾 3");
    tui.key(b"\x01");
    assert!(!tui.shows("已勾"), "{}", tui.lines().join("\n"));
    tui.key(b" ");
    tui.key(b"\x1b[A");
    tui.key(b" ");
    tui.wait_for("已勾 2");
    tui.key(CTRL_D);
    tui.wait_for("再按一次 Ctrl+D 删除 2 个会话");
    tui.key(CTRL_D);
    tui.wait_for("已删除 2 个会话");
    tui.wait_for("2 个");
    // 光标停在原来的位置（第三行），下面的补上来；只剩两个就停在最后一个。
    let shown = rows(&tui);
    assert_eq!(shown.len(), 2, "{shown:?}");
    assert!(shown[1].contains('❯'), "没跳回第一个：{shown:?}");
    // 一个一个删：没勾的删选中的那个。
    tui.key(CTRL_D);
    tui.wait_for("再按一次 Ctrl+D 删除「");
    tui.key(CTRL_D);
    tui.wait_for("已删除「");
    tui.wait_for("1 个");
}

#[test]
fn launching_without_saying_anything_leaves_no_empty_session() {
    // 2026-10-01 项目主人数到 47 个空会话：一启动就开会话，没说话就退出的留下了。说第一句话时才开（「连核心」第 4 条）。
    let home = Home::new(Script::new([Play::Says("在。")]));
    let mut idle = home.tui("zh_CN.UTF-8");
    idle.wait_for("工作区");
    idle.pump(Duration::from_millis(500));
    drop(idle);
    let mut tui = home.tui("zh_CN.UTF-8");
    tui.wait_for("工作区");
    tui.say("在吗");
    tui.wait_for("在。");
    tui.pump(Duration::from_millis(300));
    tui.say("/sessions");
    tui.wait_for("1 个");
}
