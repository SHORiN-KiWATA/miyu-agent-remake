//! 子代理状态行（蓝图 `tui.md`「后台命令、子代理和侧边栏」第 5 条，设计 13 第七节，照 Claude Code）：
//! 框下面那一行下面空一行，`● 主会话`，每个子代理一行 `○ 名字  正在做什么…  Σtoken · 用时`。

use std::time::Instant;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use unicode_width::UnicodeWidthStr;

use super::rows::clip;
use crate::app::App;
use crate::config::JobTexts;
use crate::jobs::Board;
use crate::{meter, theme};

/// 这一块占几行（连上面的空行）；没有在跑的子代理是 0。
pub fn height(board: &Board, most: usize) -> u16 {
    let n = board.agents().len();
    if n == 0 {
        return 0;
    }
    u16::try_from(2 + n.min(most) + usize::from(n > most)).unwrap_or(u16::MAX)
}

/// 每一行：空行、主会话、子代理（名字、正在做什么各对齐成一列，用时和 token 靠右）、收起来的。
/// `hover` 是鼠标悬停的第几行（第 0 行是空行），`focus` 是焦点在第几个子代理上（品红）。
pub fn lines(
    board: &Board,
    words: &JobTexts,
    width: u16,
    most: usize,
    hover: Option<usize>,
    focus: Option<usize>,
    now: Instant,
) -> Vec<Line<'static>> {
    let agents = board.agents();
    let lit = |row: usize, base: Style| {
        if focus.is_some_and(|f| f + 2 == row) {
            theme::picked()
        } else if hover == Some(row) {
            theme::hover()
        } else {
            base
        }
    };
    let mut out = vec![
        Line::raw(""),
        Line::styled(format!("  ● {}", words.main), Style::new()),
    ];
    let name_w = agents.iter().map(|a| a.title.width()).max().unwrap_or(0);
    for (i, job) in agents.iter().take(most).enumerate() {
        let stats = format!(
            "Σ{} · {}",
            meter::short(job.tokens),
            meter::clock(job.elapsed(now).as_secs())
        );
        // 每行前面两格：焦点所在的那一行写 `❯ `（第 6 条）。
        let lead = if focus == Some(i) { "❯ " } else { "  " };
        let name = format!(
            "{lead}○ {}{}",
            job.title,
            " ".repeat(name_w - job.title.width())
        );
        let room = usize::from(width).saturating_sub(name.width() + 2 + stats.width() + 2);
        let doing = clip(&job.doing, u16::try_from(room).unwrap_or(0));
        let pad = room.saturating_sub(doing.width()) + 2;
        let style = lit(i + 2, theme::dim());
        out.push(Line::from(vec![
            Span::styled(format!("{name}  {doing}{}", " ".repeat(pad)), style),
            Span::styled(stats, style),
        ]));
    }
    if agents.len() > most {
        let more = words
            .more
            .replace("{count}", &(agents.len() - most).to_string());
        out.push(Line::styled(
            format!("  {more}"),
            lit(most + 2, theme::dim()),
        ));
    }
    out
}

/// 画这一块。
pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    if area.height == 0 {
        return;
    }
    let most = app.config.layout.agent_rows;
    let lines = lines(
        &app.board,
        &app.config.text.jobs,
        area.width,
        most,
        app.agents_hover,
        match app.focus() {
            crate::focus::Focus::Agent(i) => Some(i),
            _ => None,
        },
        Instant::now(),
    );
    frame.render_widget(Paragraph::new(lines), area);
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use unicode_width::UnicodeWidthStr;

    use super::{height, lines};
    use crate::config::Config;
    use crate::jobs::{Board, Feed};

    #[test]
    fn a_blank_then_the_main_session_then_one_row_per_agent() {
        let config = Config::builtin().unwrap();
        let (mut feed, mut board) = (Feed::default(), Board::default());
        assert_eq!(height(&board, 5), 0, "没有子代理：不占地方");
        let t0 = Instant::now();
        feed.start_agent(&config.fake, &mut board, t0);
        feed.start_agent(&config.fake, &mut board, t0);
        let rows = lines(
            &board,
            &config.text.jobs,
            70,
            5,
            None,
            None,
            t0 + Duration::from_secs(65),
        );
        let text: Vec<String> = rows.iter().map(|l| l.to_string()).collect();
        assert_eq!(text[0], "");
        assert_eq!(text[1], "  ● 主会话", "前面留两格给箭头");
        assert!(
            text[2].starts_with("  ○ 查旧版缓存  读 docs/fixed"),
            "{}",
            text[2]
        );
        assert!(
            text[2].ends_with("Σ12.4k · 1m 05s"),
            "token 在用时左边：{}",
            text[2]
        );
        // 名字、正在做什么各对齐成一列，用时靠右。
        let doing = |t: &str, what: &str| t[..t.find(what).unwrap()].width();
        assert_eq!(doing(&text[2], "读"), doing(&text[3], "列"));
        assert_eq!(text[2].width(), 70);
        assert_eq!(text[3].width(), 70);
        assert_eq!(height(&board, 5), 4);
        // 超过上限的收成一行。
        assert_eq!(height(&board, 1), 4);
        let short = lines(&board, &config.text.jobs, 70, 1, None, None, t0);
        assert_eq!(short.last().unwrap().to_string(), "  还有 1 个");
        // 焦点在第二个子代理上：那一行品红。
        let focused = lines(&board, &config.text.jobs, 70, 5, None, Some(1), t0);
        assert_eq!(focused[3].spans[0].style, crate::theme::picked());
        assert!(
            focused[3].to_string().starts_with("❯ ○ 找主题映射"),
            "焦点那一行左边一个箭头"
        );
        assert!(focused[2].to_string().starts_with("  ○ 查旧版缓存"));
        assert_eq!(focused[2].spans[0].style, crate::theme::dim());
    }
}
