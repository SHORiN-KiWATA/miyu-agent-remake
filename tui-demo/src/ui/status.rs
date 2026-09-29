//! 输入框上面的一块（蓝图 `tui.md`「运行状态行和排队的消息」「提示」）：回答进行中是一个词（从词库挑，`pulse.rs`）
//! 连着一到三个点（跟着流光轮换），一起被流光扫，再是这一轮的用时和重试；下面是排队的消息；提示是浮在上面的小框。

use std::time::Instant;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};
use unicode_width::UnicodeWidthStr;

use super::rows::clip;
use crate::app::App;
use crate::config::Dots;
use crate::theme;

/// 画那一行：从词库挑的词连着一到三个点，一起被流光扫（流光明暗），补到最宽的词那么宽，再写这一轮的用时、重试。
pub fn draw(frame: &mut Frame, area: Rect, app: &mut App) {
    let Some(start) = app.transcript.running else {
        return;
    };
    let now = Instant::now();
    let beat = app.transcript.beat();
    let word = app
        .pulse
        .word(start, beat, now, &app.config.pulse)
        .to_string();
    let layout = &app.config.layout;
    // 换了词，亮光从头扫（`tui.md`「运行状态行和排队的消息」第 1 条）。
    let t = app
        .pulse
        .shown()
        .map_or(0.0, |s| now.saturating_duration_since(s).as_secs_f64());
    let chars: Vec<char> = swept(&word, &layout.dots, t, layout.shimmer.sweep_seconds)
        .chars()
        .collect();
    let mut spans: Vec<Span> = chars
        .iter()
        .enumerate()
        .map(|(i, c)| {
            Span::styled(
                c.to_string(),
                theme::shimmer(i, chars.len(), t, &layout.shimmer),
            )
        })
        .collect();
    // 补到词库里最宽的词那么宽，用时的位置不跟着词长短跳。
    let pad = app.config.pulse.widest().saturating_sub(word.width()) + 1;
    spans.push(Span::raw(" ".repeat(pad)));
    let rest = theme::shimmer_rest(&layout.shimmer);
    spans.push(Span::styled(
        crate::meter::clock(start.elapsed().as_secs()),
        rest,
    ));
    if let Some(retry) = &app.transcript.retry {
        spans.push(Span::styled(" · ", theme::dim()));
        spans.push(Span::styled(retry.clone(), theme::warn()));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// 被流光扫的那一截：词后面接着点，同一道亮光一路扫过去；点的个数跟着流光轮换，一趟亮光轮一圈，
/// 没写满的格子留空，长度不变（`tui.md`「运行状态行和排队的消息」第 2 条）。`t` 是这个词换上以后过了几秒。
fn swept(word: &str, dots: &Dots, t: f64, sweep: f64) -> String {
    let count = dots.count.max(1);
    let phase = if sweep > 0.0 {
        t.rem_euclid(sweep) / sweep
    } else {
        0.0
    };
    // 一趟分成 `count` 段，第几段就写几个点。
    let shown = ((phase * count as f64) as usize).min(count - 1) + 1;
    let gap = " ".repeat((count - shown) * dots.mark.width());
    format!("{word}{}{gap}", dots.mark.repeat(shown))
}

/// 提示：圆角细边框的小框，底边压在 `bottom` 那一行，框里的字从 `text_x` 起（和输入框里的字左对齐），
/// 左右各空一格。浮着，先把底下的格子清掉再画（`tui.md`「提示」）。
pub fn toast(frame: &mut Frame, text_x: u16, bottom: u16, max_width: u16, app: &App) {
    let Some(notice) = &app.notice else {
        return;
    };
    let text = clip(&notice.text, max_width.saturating_sub(4));
    let width = u16::try_from(text.width()).unwrap_or(0) + 4;
    let area = Rect::new(text_x.saturating_sub(2), bottom.saturating_sub(2), width, 3)
        .intersection(frame.area());
    frame.render_widget(Clear, area);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(theme::notice_border(notice.good));
    let inner =
        Rect::new(area.x + 2, area.y + 1, area.width.saturating_sub(4), 1).intersection(area);
    frame.render_widget(block, area);
    frame.render_widget(Paragraph::new(text), inner);
}

/// 排队的消息：一条一行，暗色的记号加这句话，放不下截掉加 `…`（`tui.md`「运行状态行和排队的消息」第 5 条）。
pub fn queued(frame: &mut Frame, area: Rect, texts: &[String], mark: &str) {
    let lines: Vec<Line> = texts
        .iter()
        .take(usize::from(area.height))
        .map(|t| Line::styled(clip(&format!("{mark}{t}"), area.width), theme::dim()))
        .collect();
    frame.render_widget(Paragraph::new(lines), area);
}

#[cfg(test)]
mod tests {
    use super::swept;
    use crate::config::Config;

    #[test]
    fn the_dots_are_swept_with_the_word_and_count_up_with_the_sweep() {
        let layout = Config::builtin().unwrap().layout;
        let (dots, sweep) = (&layout.dots, layout.shimmer.sweep_seconds);
        // 一趟亮光轮一圈：一个、两个、三个，再回到一个；没写满的格子留空，长度不变。
        assert_eq!(swept("思考中", dots, 0.0, sweep), "思考中.  ");
        assert_eq!(
            swept("思考中", dots, sweep / 3.0 + 0.01, sweep),
            "思考中.. "
        );
        assert_eq!(
            swept("思考中", dots, sweep * 2.0 / 3.0 + 0.01, sweep),
            "思考中..."
        );
        assert_eq!(swept("思考中", dots, sweep + 0.01, sweep), "思考中.  ");
    }
}
