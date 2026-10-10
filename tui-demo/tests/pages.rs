//! 按页读老会话（蓝图 `tui.md`「会话列表 `/sessions`」第 5 条「按页读」，核心 9-6）：进最近的那个会话只读最新一页，
//! 往上滚到顶再读更早的一页；累计用量、权限照订阅的回应，不照页里的事件。

mod support;

use std::time::Duration;

use miyu_session::testkit::{Play, Script};
use support::{Home, Tui, frames};

const PG_UP: &[u8] = b"\x1b[5~";
const TAB: &[u8] = b"\t";
/// 比一页（核心默认 20 轮）多两轮。
const TURNS: usize = 22;
/// 启动时进最近的那个会话。
const RECENT: &str = "[ui]\nstartup = \"recent\"\n";

/// 一份核心：第 i 次请求答「第 i 号的回答。」。
fn home() -> Home {
    let plays: Vec<Play> = (1..=TURNS)
        .map(|i| Play::Says(Box::leak(format!("第 {i} 号的回答。").into_boxed_str())))
        .collect();
    Home::with_settings(Script::new(plays), RECENT)
}

/// 在 `tui` 里说 [`TURNS`] 轮：第 i 轮说「第 i 句」；`after_first` 在第一轮答完以后做。
fn talk(tui: &mut Tui, after_first: impl FnOnce(&mut Tui)) {
    tui.wait_for("工作区");
    let mut after_first = Some(after_first);
    for i in 1..=TURNS {
        tui.say(&format!("第 {i} 句"));
        tui.wait_for(&format!("第 {i} 号的回答。"));
        tui.pump(Duration::from_millis(100));
        if let Some(then) = after_first.take() {
            then(tui);
        }
    }
}

/// `frame` 里写着 `text` 的是第几行。
fn row_of(frame: &[String], text: &str) -> Option<usize> {
    frame.iter().position(|l| l.contains(text))
}

#[test]
fn resuming_reads_the_latest_page_and_the_earlier_one_at_the_top_without_jumping() {
    let home = home();
    let mut first = home.tui("zh_CN.UTF-8");
    talk(&mut first, |_| {});
    drop(first);
    let mut tui = home.tui("zh_CN.UTF-8");
    tui.wait_for("第 22 号的回答。");
    tui.pump(Duration::from_millis(300));
    let start = tui.record_from_here();
    for _ in 0..40 {
        tui.key(PG_UP);
        if tui.shows("┃ 第 1 句") {
            break;
        }
    }
    let frames = frames(start, &tui.recorded());
    const MARK: &str = "正在加载更早的内容";
    // 最新一页是后 20 轮：滚到它的顶上是那一行暗色的，下面紧接着第 3 轮你说的那句。
    let marked = frames
        .iter()
        .position(|f| row_of(f, MARK).is_some())
        .unwrap_or_else(|| panic!("顶上没有「{MARK}」，最后一屏：\n{}", tui.lines().join("\n")));
    let pinned = row_of(&frames[marked], "┃ 第 3 句").expect("那一行下面是第 3 轮");
    assert!(
        row_of(&frames[marked], "第 2 号的回答。").is_none(),
        "最新一页里不该有第 2 轮：\n{}",
        frames[marked].join("\n")
    );
    // 更早的那一页拼上来：第 3 轮你说的那句还在原来那一行，视口不跳。
    let after = frames[marked..]
        .iter()
        .find(|f| row_of(f, MARK).is_none())
        .expect("更早的一页读回来了");
    assert_eq!(
        row_of(after, "┃ 第 3 句"),
        Some(pinned),
        "拼上更早的一页时视口跳了：\n{}",
        after.join("\n")
    );
    // 接着往上滚看得到第 1 轮；读到头了，顶上不再有那一行。
    assert!(tui.shows("┃ 第 1 句"), "{}", tui.lines().join("\n"));
    assert!(tui.shows("第 1 号的回答。"), "{}", tui.lines().join("\n"));
    assert!(!tui.shows(MARK), "{}", tui.lines().join("\n"));
}

#[test]
fn the_sidebar_title_total_and_level_come_from_the_whole_session_not_the_latest_page() {
    let home = home();
    let mut first = home.tui_wide("zh_CN.UTF-8", 140);
    // 第一轮答完换成开放权限、改名：这两条事件落在更早的那一页里。
    talk(&mut first, |tui| {
        tui.key(TAB);
        tui.say("/rename 很长的会话");
        tui.wait_for("已改名");
    });
    // 剧本每次请求报 110 个 token（60 没命中、40 命中、10 输出）：22 次是 2.4k，只算最新一页的 20 次是 2.2k。
    first.wait_for("共 2.4k token");
    assert!(first.shows("开放权限"), "{}", first.lines().join("\n"));
    drop(first);
    let mut tui = home.tui_wide("zh_CN.UTF-8", 140);
    tui.wait_for("第 22 号的回答。");
    tui.pump(Duration::from_millis(500));
    assert!(tui.shows("共 2.4k token"), "{}", tui.lines().join("\n"));
    assert!(tui.shows("开放权限"), "{}", tui.lines().join("\n"));
    // 标题照会话列表：最新一页里没有改名的事件。
    assert!(tui.shows("很长的会话"), "{}", tui.lines().join("\n"));
}
