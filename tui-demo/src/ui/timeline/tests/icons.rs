//! 时间线的图标（蓝图 `tui.md`「时间线」第 3 条、「图标」）：图标取当前那一套，和标题一个颜色；换一套，
//! 排好的步跟着换。

use std::cell::RefCell;
use std::time::Instant;

use super::{command, segment, text, thought};
use crate::core::ToolStatus;
use crate::theme;
use crate::transcript::{Kind, Transcript};
use crate::ui::row_cache::{RowCache, build};
use crate::ui::test_support::Fixture;
use crate::ui::timeline::rows;

#[test]
fn the_plain_set_draws_plain_icons() {
    let mut f = Fixture::new();
    f.config.icons = f
        .config
        .icon_sets
        .iter()
        .find(|s| s.name == "plain")
        .unwrap()
        .clone();
    let t0 = Instant::now();
    let mut read = command(t0, 3, ToolStatus::Ok);
    if let crate::transcript::StepKind::Tool { name, .. } = &mut read.kind {
        *name = "read".into();
    }
    let seg = segment(
        vec![
            thought(t0, 0),
            command(t0, 1, ToolStatus::Ok),
            command(t0, 2, ToolStatus::Error),
            read,
        ],
        Some(true),
    );
    let lines = text(&rows(0, &seg, &f.ctx()));
    let titles: Vec<&str> = lines
        .iter()
        .map(|l| l.trim_start())
        .filter(|l| !l.starts_with('│') && !l.is_empty())
        .skip(1)
        .collect();
    assert_eq!(
        titles,
        [
            "✳ 已思考 · 1.0s",
            "$ shell · 列目录",
            "✗ shell · 列目录",
            "⚙ read"
        ],
        "思考 ✳、执行命令 $、出错 ✗、别的工具 ⚙：{lines:?}"
    );
}

#[test]
fn another_icon_set_redraws_the_steps_already_laid_out() {
    // 数重排了几条：主题别在中途被别的测试换掉。
    let _theme = theme::hold();
    let mut f = Fixture::new();
    let mut t = Transcript::default();
    t.note(Kind::Steps, String::new());
    let t0 = Instant::now();
    let mut seg = segment(
        vec![thought(t0, 0), command(t0, 1, ToolStatus::Ok)],
        Some(true),
    );
    seg.finished = true;
    t.entries[0].segment = Some(seg);
    let cache = RefCell::new(RowCache::default());
    let before: Vec<String> = build(&t.entries, &f.ctx(), &cache)
        .iter()
        .map(|r| r.line.to_string())
        .collect();
    assert!(before.iter().any(|l| l.contains(&f.config.icons.think)));
    f.config.icons = f.config.icon_sets[1].clone();
    let after: Vec<String> = build(&t.entries, &f.ctx(), &cache)
        .iter()
        .map(|r| r.line.to_string())
        .collect();
    assert_eq!(cache.borrow().rebuilt, 1, "换了一套：排好的也重排");
    assert!(
        after.iter().any(|l| l.contains("✳ 已思考")),
        "照新的一套画：{after:?}"
    );
}

#[test]
fn icons_share_the_title_colour_and_a_thought_preview_has_its_own_colour() {
    let f = Fixture::new();
    let icons = &f.config.icons;
    let t0 = Instant::now();
    let seg = segment(
        vec![thought(t0, 0), command(t0, 1, ToolStatus::Ok)],
        Some(true),
    );
    let rows = rows(0, &seg, &f.ctx());
    let icon_style = |row: usize, icon: &str| {
        rows[row]
            .line
            .spans
            .iter()
            .find(|s| s.content.starts_with(icon))
            .map(|s| s.style)
    };
    // 第 2 行思考的标题、第 3 行它的预览、第 5 行命令的标题（`tui.md`「时间线」第 3、5 条）。
    assert_eq!(
        icon_style(2, &icons.think),
        Some(theme::dim()),
        "图标和字一个颜色"
    );
    assert_eq!(icon_style(5, icons.tool("shell")), Some(theme::dim()));
    let preview = rows[3].line.spans.last().unwrap();
    assert_eq!(preview.content, "先看看目录");
    assert_eq!(
        preview.style,
        theme::thought(),
        "用主题的 thought，和命令预览分开"
    );
    assert_ne!(theme::thought(), theme::dim());
    // 竖线照旧暗；悬停时和别的步一样亮一档。
    assert!(
        rows[3]
            .line
            .spans
            .iter()
            .any(|s| s.content == "│ " && s.style == theme::dim())
    );
    let mut ctx = f.ctx();
    ctx.hover = Some(crate::ui::rows::Target::Step(0, 0));
    let lit = crate::ui::timeline::rows(0, &seg, &ctx);
    assert_eq!(
        lit[3].line.spans.last().unwrap().style,
        theme::thought_hover(),
        "悬停亮成同色系，不变灰"
    );
    assert_ne!(theme::thought_hover(), theme::hover());
}
