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
fn a_recap_leads_with_a_dim_reference_mark_and_folds_under_its_words() {
    // 2026-10-01 项目主人：回顾不要行首的粗竖线，换成暗色的 `※`。
    use crate::transcript::{Kind, Transcript};
    use crate::ui::test_support::Fixture;
    let f = Fixture::new();
    let mut ctx = f.ctx();
    ctx.width = 20;
    let mut t = Transcript::default();
    t.note(Kind::Recap, "回顾：在做一件很长很长很长很长的事。".into());
    let rows = super::entry_rows(0, &t.entries[0], &ctx);
    assert!(rows.len() > 1, "窄了要折行");
    let mark = rows[0]
        .line
        .spans
        .iter()
        .find(|s| s.content == "※ ")
        .unwrap();
    assert_eq!(mark.style, theme::dim(), "记号暗");
    assert!(
        rows.iter().all(|r| !r.line.to_string().contains('┃')),
        "没有竖线"
    );
    assert!(
        rows[1].line.to_string().contains("  "),
        "折下来的行和字对齐"
    );
    assert!(
        !rows[0].plain.contains('※'),
        "复制时不带记号：{}",
        rows[0].plain
    );
}

#[test]
fn a_new_theme_redraws_cached_replies() {
    let _theme = theme::hold();
    let before = cache_key("**粗**", &[], "zh");
    // 设回同一套：颜色不变（不扰别的测试），但换过一次，排好的样子就作废。
    let palette = theme::builtin().unwrap().remove(0).1;
    theme::set(palette);
    assert_ne!(cache_key("**粗**", &[], "zh"), before);
}

#[test]
fn an_opened_undo_lists_each_file_and_opens_a_diff_on_click() {
    // 2026-10-02 项目主人：撤销改回了文件时，要看得到是哪几个、改了什么；「命令的改动撤不回」不要。
    use crate::core::{Report, UndoFile};
    use crate::transcript::{Kind, Transcript};
    use crate::ui::rows::Target;
    use crate::ui::test_support::Fixture;
    let f = Fixture::new();
    let mut t = Transcript::default();
    t.note(Kind::Undo, "改一下".into());
    t.entries[0].undo = Some(Report {
        turns: 1,
        cwd: Some("/w".into()),
        files: vec![
            UndoFile {
                path: "/w/src/a.rs".into(),
                outcome: "restored".into(),
                diff: vec!["@@ -1 +1 @@".into(), "-new".into(), "+old".into()],
                ..Default::default()
            },
            UndoFile {
                path: "/w/README.md".into(),
                outcome: "changed".into(),
                ..Default::default()
            },
            // 同一个文件的另一步（先写、再编辑）：并进上面那一行。
            UndoFile {
                path: "/w/src/a.rs".into(),
                outcome: "restored".into(),
                ..Default::default()
            },
        ],
        ..Default::default()
    });
    let rows = super::entry_rows(0, &t.entries[0], &f.ctx());
    let head = rows[0].line.to_string();
    assert!(
        head.contains("已撤销 · 改回 1 个文件 · /restore 恢复 · 改一下"),
        "{head}"
    );
    t.entries[0].open = true;
    let rows = super::entry_rows(0, &t.entries[0], &f.ctx());
    let text: Vec<String> = rows.iter().map(|r| r.line.to_string()).collect();
    let a = text
        .iter()
        .position(|l| l.contains("✓ src/a.rs  已改回  +1 −1"));
    assert!(a.is_some(), "{text:#?}");
    assert!(
        text.iter()
            .any(|l| l.contains("✗ README.md  没动：之后又被改过")),
        "{text:#?}"
    );
    assert!(!text.iter().any(|l| l.contains("命令")), "{text:#?}");
    assert_eq!(
        text.iter().filter(|l| l.contains("src/a.rs")).count(),
        1,
        "同一个文件一行：{text:#?}"
    );
    let a = a.unwrap();
    assert_eq!(
        rows[a].target,
        Some(Target::Details(0, 0)),
        "有差异的那一行点了展开差异"
    );
    assert!(!text.iter().any(|l| l.contains("- new")), "没点开不露差异");
    t.entries[0].details.push(0);
    let rows = super::entry_rows(0, &t.entries[0], &f.ctx());
    let text: Vec<String> = rows.iter().map(|r| r.line.to_string()).collect();
    assert!(
        text.iter().any(|l| l.contains("- new")) && text.iter().any(|l| l.contains("+ old")),
        "{text:#?}"
    );
}

/// 从真实的用户消息排版到选区提取，不能把标签当作原文。
#[test]
fn mouse_copy_expands_chips_once_across_wraps() {
    use crate::body_view::BodyView;
    use crate::transcript::{Chip, Kind, Transcript};
    use crate::ui::test_support::Fixture;
    let f = Fixture::new();
    let mut t = Transcript::default();
    t.note(Kind::User, "前[已粘贴 14 行]中[已粘贴 14 行]后".into());
    t.entries[0].pasted = vec![
        Chip {
            label: "[已粘贴 14 行]".into(),
            full: "第一段\n原文".into(),
            kind: None,
            file: None,
        },
        Chip {
            label: "[已粘贴 14 行]".into(),
            full: "第二段".into(),
            kind: None,
            file: None,
        },
    ];
    for width in [1, 2, 6, 60] {
        let mut ctx = f.ctx();
        ctx.width = width;
        let rows = super::entry_rows(0, &t.entries[0], &ctx);
        let mut view = BodyView::default();
        view.rows = rows.into();
        view.select = Some(((1, 0), (view.rows.len() - 2, 100)));
        assert_eq!(
            view.selected_text(),
            "前第一段\n原文中第二段后",
            "宽度 {width}"
        );
        view.select = view.select.map(|(a, b)| (b, a));
        assert_eq!(view.selected_text(), "前第一段\n原文中第二段后");
    }
    let rows = super::entry_rows(0, &t.entries[0], &f.ctx());
    let mut view = BodyView::default();
    view.rows = rows.into();
    view.select = Some(((1, 6), (1, 7)));
    assert_eq!(
        view.selected_text(),
        "第一段\n原文",
        "选到半个标签也展开整块"
    );
    t.entries[0].open = true;
    let rows = super::entry_rows(0, &t.entries[0], &f.ctx());
    let mut view = BodyView::default();
    view.rows = rows.into();
    view.select = Some(((1, 4), (1, 5)));
    assert_eq!(view.selected_text(), "第", "展开后按实际选区复制");
}

#[test]
fn mouse_copy_expands_file_paths_but_not_literal_labels() {
    use crate::body_view::BodyView;
    use crate::transcript::{Chip, Kind, Transcript};
    use crate::ui::test_support::Fixture;
    let f = Fixture::new();
    let mut t = Transcript::default();
    t.note(Kind::User, "[文件 1] [已粘贴 14 行]".into());
    t.entries[0].pasted = vec![Chip {
        label: "[文件 1]".into(),
        full: "/tmp/中文文件.txt".into(),
        kind: None,
        file: Some("/tmp/中文文件.txt".into()),
    }];
    let rows = super::entry_rows(0, &t.entries[0], &f.ctx());
    let mut view = BodyView::default();
    view.rows = rows.into();
    view.select = Some(((1, 0), (view.rows.len() - 2, 100)));
    assert_eq!(view.selected_text(), "/tmp/中文文件.txt [已粘贴 14 行]");
}

#[test]
fn mouse_copy_keeps_attachment_labels_and_separates_entries() {
    use crate::body_view::BodyView;
    use crate::transcript::{Chip, Kind, Transcript};
    use crate::ui::test_support::{Fixture, fresh_rows};
    let f = Fixture::new();
    let mut t = Transcript::default();
    for full in ["一", "二"] {
        t.note(Kind::User, "[已粘贴 14 行][图片 1]".into());
        t.entries.last_mut().unwrap().pasted = vec![
            Chip {
                label: "[已粘贴 14 行]".into(),
                full: full.into(),
                kind: None,
                file: None,
            },
            Chip {
                label: "[图片 1]".into(),
                full: "[图片 1]".into(),
                kind: Some("image".into()),
                file: Some("/tmp/a.png".into()),
            },
        ];
    }
    let mut view = BodyView::default();
    view.rows = fresh_rows(&t.entries, &f.ctx()).into();
    view.select = Some(((1, 0), (view.rows.len() - 2, 100)));
    assert_eq!(view.selected_text(), "一[图片 1]\n\n\n\n二[图片 1]");
}

#[test]
fn mouse_copy_preserves_whitespace_inside_expanded_original() {
    use crate::body_view::BodyView;
    use crate::transcript::{Chip, Kind, Transcript};
    use crate::ui::test_support::Fixture;
    let f = Fixture::new();
    let mut t = Transcript::default();
    t.note(Kind::User, "[已粘贴 14 行]".into());
    let full = "正文末尾的空格  \n\n";
    t.entries[0].pasted = vec![Chip {
        label: "[已粘贴 14 行]".into(),
        full: full.into(),
        kind: None,
        file: None,
    }];
    for width in [2, 60] {
        let mut ctx = f.ctx();
        ctx.width = width;
        let mut view = BodyView::default();
        view.rows = super::entry_rows(0, &t.entries[0], &ctx).into();
        view.select = Some(((1, 0), (view.rows.len() - 2, 100)));
        assert_eq!(view.selected_text(), full);
    }
}
