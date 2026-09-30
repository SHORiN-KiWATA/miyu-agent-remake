//! 正文里后台任务结束的那一行（蓝图 `tui.md`「后台命令、子代理和侧边栏」第 5 条）：记号有颜色、字是原色；
//! 有全文（子代理的报告、命令和它的输出）的能点开：整块铺底色，那一行连同全文，照时间线点开一步。

use ratatui::style::{Modifier, Style};
use ratatui::text::Span;

use super::rows::{Ctx, Row, Target, clip};
use crate::input::pieces;
use crate::theme;
use crate::transcript::{Entry, JobMark};

/// 排成的行。`i` 是它在正文里是第几条。
pub fn rows(i: usize, entry: &Entry, ctx: &Ctx) -> Vec<Row> {
    let Some(job) = &entry.job else {
        return Vec::new();
    };
    let words = &ctx.config.text.jobs;
    let target = Target::Entry(i);
    let can_open = !job.detail.trim().is_empty();
    let hovered = can_open && ctx.hover == Some(target);
    let lit = |s: Style| {
        if hovered {
            s.add_modifier(Modifier::BOLD)
        } else {
            s
        }
    };
    let (mark, mark_style, text_style) = match job.mark {
        JobMark::Done => (words.ok_mark.as_str(), theme::good(), theme::dim()),
        JobMark::Failed => (words.fail_mark.as_str(), theme::error(), theme::dim()),
        JobMark::Stopped => (words.stop_mark.as_str(), theme::dim(), theme::dim()),
    };
    let width = ctx
        .width
        .saturating_sub(u16::try_from(mark.chars().count() + 1).unwrap_or(0));
    let mut head = ctx.row(
        ctx.blank_slot(),
        vec![
            Span::styled(mark.to_string(), mark_style),
            Span::styled(clip(&entry.text, width), lit(text_style)),
        ],
    );
    if !can_open {
        return vec![head];
    }
    head.target = Some(target);
    let mut out = vec![head];
    if entry.open {
        let width = ctx.width.saturating_sub(2).max(1);
        let blank = || ctx.row(ctx.blank_slot(), Vec::new());
        out.push(blank());
        for (piece, joined) in pieces(job.detail.trim_matches('\n'), width) {
            let mut row = ctx.led_row(
                ctx.blank_slot(),
                vec![Span::raw("  ")],
                vec![Span::raw(piece)],
            );
            row.joined = joined;
            out.push(row);
        }
        out.push(blank());
        // 点开是整块换成铺底色的，连那一行一起（照时间线点开一步；2026-09-30 项目主人：原来那一行没底色，看着怪）。
        for row in &mut out {
            row.shade = true;
            row.target = Some(target);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::rows;
    use crate::theme;
    use crate::transcript::{JobMark, Transcript};
    use crate::ui::rows::Target;
    use crate::ui::test_support::Fixture;

    #[test]
    fn the_mark_is_coloured_and_the_words_are_plain() {
        let f = Fixture::new();
        let mut t = Transcript::default();
        t.job(
            JobMark::Done,
            "后台命令完成 · ls · 2s".into(),
            String::new(),
        );
        t.job(
            JobMark::Failed,
            "后台命令失败 · make · 退出码 1".into(),
            String::new(),
        );
        let done = rows(0, &t.entries[0], &f.ctx());
        assert_eq!(
            done[0].line.to_string().trim(),
            "● 后台命令完成 · ls · 2s",
            "2026-09-30 项目主人：绿勾换成绿点"
        );
        let spans = &done[0].line.spans;
        assert!(
            spans
                .iter()
                .any(|s| s.content == "● " && s.style == theme::good())
        );
        assert!(
            spans
                .iter()
                .any(|s| s.content.starts_with("后台命令完成") && s.style == theme::dim()),
            "2026-09-30 项目主人：字改暗，不要白色的字"
        );
        assert!(done[0].target.is_none(), "没有全文的不能点");
        let failed = rows(1, &t.entries[1], &f.ctx());
        assert!(
            failed[0]
                .line
                .spans
                .iter()
                .any(|s| s.content == "✗ " && s.style == theme::error())
        );
    }

    #[test]
    fn a_stopped_note_has_a_dim_dot() {
        let f = Fixture::new();
        let mut t = Transcript::default();
        t.job(
            JobMark::Stopped,
            "后台命令已停止 · sleep 300".into(),
            String::new(),
        );
        let stopped = rows(0, &t.entries[0], &f.ctx());
        assert_eq!(
            stopped[0].line.to_string().trim(),
            "● 后台命令已停止 · sleep 300",
            "2026-09-30 项目主人：停了的也加实心圆点，和别的几行对齐"
        );
        assert!(
            stopped[0]
                .line
                .spans
                .iter()
                .any(|s| s.content == "● " && s.style == theme::dim()),
            "圆点是暗的"
        );
    }

    #[test]
    fn a_report_opens_underneath_on_a_shade() {
        let f = Fixture::new();
        let mut t = Transcript::default();
        t.job(
            JobMark::Done,
            "后台任务完成 · 查缓存 · 20s".into(),
            "查完了。\n\n结论在这。".into(),
        );
        let closed = rows(0, &t.entries[0], &f.ctx());
        assert_eq!(closed.len(), 1);
        assert_eq!(closed[0].target, Some(Target::Entry(0)), "能点");
        t.entries[0].open = true;
        let open = rows(0, &t.entries[0], &f.ctx());
        let text: Vec<String> = open
            .iter()
            .map(|r| r.line.to_string().trim_end().to_string())
            .collect();
        assert_eq!(text[1..], ["", "    查完了。", "", "    结论在这。", ""]);
        assert!(
            open.iter().all(|r| r.shade),
            "整块铺底色，连那一行一起；全文和那一行的字对齐"
        );
    }

    #[test]
    fn command_output_keeps_its_leading_spaces() {
        let f = Fixture::new();
        let mut t = Transcript::default();
        let output = "   Compiling a\n   Compiling b".to_string();
        t.job(JobMark::Done, "后台命令完成 · cargo · 9s".into(), output);
        t.entries[0].open = true;
        let open = rows(0, &t.entries[0], &f.ctx());
        let text: Vec<String> = open
            .iter()
            .map(|r| r.line.to_string().trim_end().to_string())
            .collect();
        assert_eq!(text[2], text[3].replace('b', "a"), "第一行的缩进不被吃掉");
    }
}
