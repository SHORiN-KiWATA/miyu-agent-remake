//! 伪终端里的端到端测试：花了多少钱（蓝图 `tui.md`「配置与模型」第 5 条，核心 8-15）。剧本照价格算好金额写进
//! `model.called` 的 `cost`，侧边栏、框下面那一行照它写；`/usage` 向核心要 `usage.query`。

mod support;

use std::time::Duration;

use miyu_models::catalog::{Price, Rates};
use miyu_models::price::Tariff;
use miyu_session::testkit::{Play, Script};
use support::Home;

/// 输入 1、输出 2、读缓存 0.5 美元每百万：剧本一次报 60 没命中、40 命中、10 输出，花 0.0001。
fn usd() -> Tariff {
    Tariff {
        price: Price::of(
            Rates {
                input: Some(1.0),
                output: Some(2.0),
                cache_read: Some(0.5),
                cache_write: None,
            },
            "USD",
        ),
        multiplier: 1.0,
        source: "config:system/config.toml:7".to_string(),
    }
}

#[test]
fn the_sidebar_and_the_footer_show_what_was_spent_and_slash_usage_lists_it() {
    let home = Home::new(Script::new([Play::Says("好。")]).priced(usd()));
    let mut tui = home.tui_wide("zh_CN.UTF-8", 140);
    tui.wait_for("工作区");
    tui.say("在吗");
    tui.wait_for("好。");
    tui.wait_for("花费 $0.0001");
    tui.wait_for("· $0.0001");
    tui.say("/usage");
    // 总览：四个数、热度图（2026-10-07 项目主人：照 Codex 做细）。
    tui.wait_for("单日最高");
    tui.wait_for("连续使用");
    tui.wait_for("1 天");
    tui.wait_for("最长的会话");
    tui.wait_for("共 $0.0001");
    assert!(tui.shows("少") && tui.shows("多"), "热度图的图例");
    assert!(tui.shows("■"), "热度图的格子");
    // 换页不改框的高度（2026-10-07 项目主人）。
    let height = |tui: &support::Tui| {
        let lines = tui.lines();
        let top = lines.iter().position(|l| l.contains("╭─ 用量"));
        let bottom = lines.iter().position(|l| l.contains("╰─ Tab"));
        top.zip(bottom).map(|(t, b)| b - t)
    };
    let first = height(&tui);
    // 最近 30 天：柱状图加按天的表。
    tui.key(b"\t");
    tui.wait_for("今天");
    tui.wait_for("1 次");
    assert!(tui.shows("█"), "柱状图");
    // 按模型：占比条、用途。
    tui.key(b"\t");
    tui.wait_for("100%");
    tui.wait_for("主对话 100%");
    let row = tui
        .lines()
        .into_iter()
        .find(|l| l.contains("100%"))
        .unwrap_or_default();
    assert!(row.contains("1 次   110   $0.0001"), "{row}");
    assert_eq!(height(&tui), first, "按模型那一页矮，框不变矮");
    // 按会话：没标题的照预览（第一句话）。
    tui.key(b"\t");
    tui.wait_for("在吗");
    assert_eq!(height(&tui), first);
    // 换回总览：`Shift+Tab` 三下。
    for _ in 0..3 {
        tui.key(b"\x1b[Z");
    }
    tui.wait_for("单日最高");
    tui.key(b"\x1b");
}

#[test]
fn requests_without_a_price_are_counted_not_guessed() {
    let home = Home::new(Script::new([Play::Says("好。")]));
    let mut tui = home.tui_wide("zh_CN.UTF-8", 140);
    tui.wait_for("工作区");
    tui.say("在吗");
    tui.wait_for("好。");
    tui.wait_for("缓存命中");
    tui.pump(Duration::from_millis(300));
    let screen = tui.lines().join("\n");
    assert!(
        !screen.contains("花费"),
        "一次都算不出的不写花费：\n{screen}"
    );
    // 2026-10-10 项目主人：「没有费用记录的请求不需要出现在侧边栏中」。
    assert!(!screen.contains("没有价格"), "{screen}");
    assert!(!screen.contains('$'), "框下面那一行也不写：\n{screen}");
}
