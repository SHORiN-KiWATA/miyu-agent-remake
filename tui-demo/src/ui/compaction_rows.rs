//! 正在压缩的那一行（蓝图 `tui.md`「正文」第 9 条）：行首不留东西，「正在压缩上下文」连着一到三个点，照运行状态行
//! 一起被流光扫，后面是已经写了多少字；下面一行进度条和字对齐：已经亮的静着，最前面那一格明暗来回，亮几格照
//! [`Progress::lit`]（按整格一顿一顿地追真实的字数），百分比照真实的字数。这次压缩过一会儿，字和条一起往白里呼吸。
//! 核心没给估计要写多少字的、自动压缩停下来等的（行首绿点），不画进度条。

use std::f64::consts::TAU;
use std::time::Duration;

use ratatui::text::Span;

use super::rows::{Ctx, Row};
use crate::config::CompactionMotion;
use crate::transcript::Entry;
use crate::{meter, theme};

/// 排成的行：流光的字加数字，再一行进度条。进度条是引子，复制时不带。
pub fn rows(entry: &Entry, ctx: &Ctx) -> Vec<Row> {
    let layout = &ctx.config.layout;
    let texts = &ctx.config.text.compaction;
    // 转圈的帧数折成秒：流光、闪、呼吸和转圈照同一个钟走。
    let t = ctx.frame as f64 * ctx.config.timeline.spinner_ms as f64 / 1000.0;
    let glow = entry.progress.as_ref().map_or(0.0, |p| {
        breath(p.since.elapsed(), &layout.compaction) * layout.compaction.breathe_lift
    });
    // 字连着点，和运行状态行一样一起被流光扫（「运行状态行和排队的消息」第 2 条）。
    let swept = super::status::swept(
        &texts.progress,
        &layout.dots,
        t,
        layout.shimmer.sweep_seconds,
    );
    let chars: Vec<char> = swept.chars().collect();
    // 自动压缩停下来等的那一行行首绿点（「正文」第 9 条）。
    let mut spans: Vec<Span<'static>> = entry
        .mark
        .iter()
        .map(|mark| Span::styled(mark.clone(), theme::good()))
        .collect();
    spans.extend(chars.iter().enumerate().map(|(i, c)| {
        let style = theme::shimmer(i, chars.len(), t, &layout.shimmer);
        Span::styled(c.to_string(), theme::lifted(style, glow))
    }));
    // 还一个字都没写的不写 0（「正文」第 9 条）。
    if let Some(progress) = entry.progress.as_ref().filter(|p| p.written > 0) {
        let count = texts
            .written
            .replace("{written}", &meter::thousands(progress.written));
        spans.push(Span::styled(count, theme::shimmer_rest(&layout.shimmer)));
    }
    let mut out = vec![ctx.row(ctx.blank_slot(), spans)];
    if let Some(progress) = &entry.progress
        && let Some((written, expected)) = progress.capped(&layout.compaction)
    {
        // 压好了、在走满：百分比跟着格子走到 100，不比压好那一刻的小（只往上走）。
        let real = (written * 100 + expected / 2) / expected;
        let percent = match progress.finish {
            Some(_) => real.max((progress.lit * 100 / layout.bar.width.max(1)) as u64),
            None => real,
        };
        out.push(ctx.led_row(
            ctx.blank_slot(),
            bar(progress.lit, percent, t, glow, ctx),
            Vec::new(),
        ));
    }
    out
}

/// 进度条和百分比：亮 `lit` 格，最前面那一格明暗来回。
fn bar(lit: usize, percent: u64, t: f64, glow: f64, ctx: &Ctx) -> Vec<Span<'static>> {
    let marks = &ctx.config.layout.bar;
    let look = &ctx.config.layout.compaction;
    let width = marks.width;
    let lit = lit.min(width);
    let k = ((t * TAU * look.blink_hz).sin() + 1.0) / 2.0;
    let lift = ctx.config.layout.shimmer.lift;
    let mut spans = Vec::new();
    if lit > 1 {
        spans.push(Span::styled(
            marks.full.repeat(lit - 1),
            theme::lifted(theme::accent(), glow),
        ));
    }
    if lit > 0 {
        let front = theme::frontier(k, look.blink_low, lift);
        spans.push(Span::styled(marks.full.clone(), theme::lifted(front, glow)));
    }
    spans.push(Span::styled(marks.empty.repeat(width - lit), theme::dim()));
    let text = &ctx.config.text.compaction.percent;
    spans.push(Span::styled(
        text.replace("{percent}", &percent.to_string()),
        theme::dim(),
    ));
    spans
}

/// 呼吸有多深，0 到 1：这次压缩过了 `breathe_after_ms` 才开始，从 0 慢慢起，`breathe_period_ms` 一个来回
/// （照 Claude Code 2.1.280 运行状态行的呼吸；从 0 起，不一下跳到一半）。
fn breath(elapsed: Duration, look: &CompactionMotion) -> f64 {
    let after = Duration::from_millis(look.breathe_after_ms);
    let Some(into) = elapsed.checked_sub(after) else {
        return 0.0;
    };
    let period = look.breathe_period_ms.max(1) as f64 / 1000.0;
    (1.0 - (into.as_secs_f64() / period * TAU).cos()) / 2.0
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use ratatui::style::Modifier;

    use super::breath;
    use crate::config::Config;
    use crate::transcript::{Kind, Progress, Transcript};
    use crate::ui::rows::entry_rows;
    use crate::ui::test_support::Fixture;

    fn rows(written: u64, lit: usize, expected: Option<u64>) -> Vec<crate::ui::rows::Row> {
        let f = Fixture::new();
        let mut t = Transcript::default();
        t.note(Kind::Note, "正在压缩上下文 3,120".into());
        let mut p = Progress::new(written, expected, Instant::now());
        p.lit = lit;
        t.entries[0].progress = Some(p);
        entry_rows(0, &t.entries[0], &f.ctx())
    }

    fn text(rows: &[crate::ui::rows::Row]) -> Vec<String> {
        rows.iter()
            .map(|r| r.line.to_string().trim_end().to_string())
            .collect()
    }

    #[test]
    fn no_spinner_the_words_and_dots_then_the_number_and_a_bar_below() {
        let config = Config::builtin().unwrap();
        let bar = &config.layout.bar;
        let got = text(&rows(3120, 3, Some(20000)));
        // 第 0 帧：一个点，另两格留空，数字不跟着跳。
        assert_eq!(
            got[0], "  正在压缩上下文.   3,120",
            "行首不留东西，字从槽后面起"
        );
        // 亮 3 格；百分比照真实的 3,120 字是 16%；和字对齐，条和百分比之间是不断行空格。
        assert_eq!(
            got[1],
            format!(
                "  {}{}\u{a0}16%",
                bar.full.repeat(3),
                bar.empty.repeat(bar.width - 3)
            )
        );
        assert_eq!(got.len(), 2);
    }

    #[test]
    fn the_words_shimmer_the_number_does_not_and_the_front_cell_blinks() {
        let f = Fixture::new();
        let got = rows(8000, 8, Some(20000));
        let spans = &got[0].line.spans;
        let first = spans.iter().find(|s| s.content == "正").unwrap();
        assert!(
            first.style.add_modifier.contains(Modifier::BOLD),
            "流光的字加粗"
        );
        let number = spans.iter().find(|s| s.content.contains("8,000")).unwrap();
        assert_eq!(
            number.style,
            crate::theme::shimmer_rest(&f.config.layout.shimmer)
        );
        // 条：前面亮着的一段是强调色，最前面那一格单独一段、颜色另算。
        let bar = &got[1].line.spans;
        let full = &f.config.layout.bar.full;
        let lit: Vec<_> = bar
            .iter()
            .filter(|s| s.content.contains(full.as_str()))
            .collect();
        assert_eq!(lit.len(), 2, "前面一段、最前面一格");
        assert_eq!(lit[1].content, *full);
        assert_ne!(lit[0].style, lit[1].style, "最前面那一格在闪");
    }

    #[test]
    fn while_filling_the_percent_only_goes_up() {
        // 2026-09-29 实测：压好那一刻百分比从 23% 掉到 20%（按亮着的 4 格算）。
        let f = Fixture::new();
        let mut t = Transcript::default();
        t.note(Kind::Note, "正在压缩上下文 4,600".into());
        let mut p = Progress::new(4_600, Some(20_000), Instant::now());
        p.lit = 4;
        p.done(Instant::now(), "上下文已压缩".into(), "● ".into());
        t.entries[0].progress = Some(p);
        let got = text(&entry_rows(0, &t.entries[0], &f.ctx()));
        assert!(got[1].ends_with("\u{a0}23%"), "{:?}", got[1]);
    }

    #[test]
    fn nothing_lit_yet_and_no_estimate_means_no_bar() {
        let config = Config::builtin().unwrap();
        let got = text(&rows(0, 0, Some(20000)));
        // 2026-09-29 项目主人经施工会话转告：还一个字都没写，不写 0。
        assert_eq!(got[0], "  正在压缩上下文.", "0 不显示");
        assert!(got[1].contains(&config.layout.bar.empty.repeat(config.layout.bar.width)));
        assert!(got[1].ends_with("\u{a0}0%"));
        assert_eq!(rows(3120, 0, None).len(), 1, "核心没给估计：只有那一句");
    }

    #[test]
    fn breathing_starts_after_a_while_and_rises_from_nothing() {
        let look = Config::builtin().unwrap().layout.compaction;
        let after = Duration::from_millis(look.breathe_after_ms);
        let period = Duration::from_millis(look.breathe_period_ms);
        assert_eq!(breath(after / 2, &look), 0.0, "刚开始压：不呼吸");
        assert!(breath(after, &look) < 0.01, "开始呼吸时从 0 起，不跳到一半");
        assert!(
            (breath(after + period / 2, &look) - 1.0).abs() < 1e-9,
            "半个来回最深"
        );
    }
}
