//! 正文排成的行（`ui/rows.rs`）。

use super::cache_key;
use crate::theme;

#[test]
fn a_turn_cut_off_by_the_core_looks_like_an_interrupted_one_in_red() {
    use crate::core::Level;
    use crate::transcript::{Kind, Transcript};
    use crate::ui::test_support::Fixture;
    let f = Fixture::new();
    let mut t = Transcript::default();
    t.note(Kind::Cut, "核心断开连接".into());
    t.entries[0].level = Some(Level::Workspace);
    let rows = super::entry_rows(0, &t.entries[0], &f.ctx());
    let icon = &f.config.layout.level_icons[&Level::Workspace];
    let text = rows[0].line.to_string();
    assert!(
        text.trim_start().starts_with(icon.as_str()) && text.ends_with("核心断开连接"),
        "{text}"
    );
    let words = rows[0]
        .line
        .spans
        .iter()
        .find(|s| s.content.contains("断开"));
    assert_eq!(words.unwrap().style.fg, theme::error().fg, "整行红");
}

#[test]
fn the_undo_line_just_says_undone() {
    // 2026-09-29 项目主人：撤销只能一轮一轮撤，写几轮没意义。
    use crate::transcript::{Kind, Transcript};
    use crate::ui::test_support::Fixture;
    let f = Fixture::new();
    let mut t = Transcript::default();
    t.note(Kind::Undo, "第一行".into());
    t.entries[0].undo = Some(crate::core::Report {
        turns: 1,
        ..Default::default()
    });
    let rows = super::entry_rows(0, &t.entries[0], &f.ctx());
    let head = rows[0].line.to_string();
    assert!(
        head.contains("已撤销 · /restore 恢复 · 第一行") && !head.contains("轮"),
        "{head}"
    );
}

#[test]
fn an_opened_undo_line_is_one_shaded_block_with_its_head() {
    // 2026-09-30 项目主人：点开是整块换成铺底色的，连那一行一起（和后台任务结束的那一行一样）。
    use crate::transcript::{Kind, Transcript};
    use crate::ui::test_support::Fixture;
    let f = Fixture::new();
    let mut t = Transcript::default();
    t.note(Kind::Undo, "第一行".into());
    t.entries[0].undo = Some(crate::core::Report {
        turns: 1,
        ..Default::default()
    });
    t.entries[0].open = true;
    let rows = super::entry_rows(0, &t.entries[0], &f.ctx());
    assert!(rows.len() > 2);
    assert!(rows.iter().all(|r| r.shade), "连那一行一起铺底色");
}

#[test]
fn undoing_a_clear_says_so_under_the_undo_line() {
    // 施工 6-8 补：撤掉的几轮里有清空，照 `miyu undo` 说一句。
    use crate::transcript::{Kind, Transcript};
    use crate::ui::test_support::Fixture;
    let f = Fixture::new();
    let mut t = Transcript::default();
    t.note(Kind::Undo, "第一行".into());
    t.entries[0].undo = Some(crate::core::Report {
        turns: 1,
        clears: 1,
        ..Default::default()
    });
    let rows = super::entry_rows(0, &t.entries[0], &f.ctx());
    let lines: Vec<String> = rows.iter().map(|r| r.line.to_string()).collect();
    assert!(
        lines
            .iter()
            .any(|l| l.contains("撤掉了清空，上下文回到了清空以前")),
        "{lines:?}"
    );
}

#[test]
fn undoing_a_compaction_says_so_under_the_undo_line() {
    // 施工 6-9：撤掉的几轮里有压缩，撤销那一行下面说一句（`tui.md`「正文」第 5 条，照 `miyu undo`）。
    use crate::transcript::{Kind, Transcript};
    use crate::ui::test_support::Fixture;
    let f = Fixture::new();
    let mut t = Transcript::default();
    t.note(Kind::Undo, "第一行".into());
    t.entries[0].undo = Some(crate::core::Report {
        turns: 1,
        compactions: 2,
        ..Default::default()
    });
    let rows = super::entry_rows(0, &t.entries[0], &f.ctx());
    let lines: Vec<String> = rows.iter().map(|r| r.line.to_string()).collect();
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert_eq!(
        lines[1].trim(),
        "撤掉了压缩，上下文回到了压缩前",
        "几次都是这一句"
    );
    assert_eq!(
        rows[1].target, rows[0].target,
        "和撤销那一行是一条，点它一样点开"
    );
    t.entries[0].undo.as_mut().unwrap().compactions = 0;
    assert_eq!(
        super::entry_rows(0, &t.entries[0], &f.ctx()).len(),
        1,
        "没撤掉压缩不说"
    );
}

#[test]
fn a_compacted_line_has_a_green_dot_that_is_not_copied() {
    // 2026-09-29 项目主人：压好了那一行前面的 `·` 换成绿色的实心圆点。
    use crate::transcript::{Kind, Transcript};
    use crate::ui::test_support::Fixture;
    let f = Fixture::new();
    let mut t = Transcript::default();
    t.note(Kind::Note, "上下文已压缩：12.3k → 4k token".into());
    t.entries[0].mark = Some("● ".into());
    let rows = super::entry_rows(0, &t.entries[0], &f.ctx());
    let spans = &rows[0].line.spans;
    let dot = spans.iter().find(|s| s.content == "● ").unwrap();
    assert_eq!(dot.style, theme::good(), "绿");
    let words = spans
        .iter()
        .find(|s| s.content.starts_with("上下文"))
        .unwrap();
    assert_eq!(words.style, theme::dim(), "字暗");
    assert!(
        !rows[0].plain.contains('●'),
        "复制时不带记号：{}",
        rows[0].plain
    );
}

#[test]
fn a_new_theme_redraws_cached_replies() {
    let _theme = theme::hold();
    let before = cache_key("**粗**", &[]);
    // 设回同一套：颜色不变（不扰别的测试），但换过一次，排好的样子就作废。
    let palette = theme::builtin().unwrap().remove(0).1;
    theme::set(palette);
    assert_ne!(cache_key("**粗**", &[]), before);
}
