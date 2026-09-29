//! 后台面板（蓝图 `tui.md`「后台命令、子代理和侧边栏」第 3 条，照 Claude Code）：和命令列表
//! 同一个位置。后台面板里点开一条，照时间线「点开一步」的样子在它下面铺底色展开输出。

use std::time::Instant;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use super::panel::Chrome;
use super::rows::clip;
use crate::app::Panel;
use crate::config::Config;
use crate::jobs::{Board, Job, JobState};
use crate::{meter, theme};

/// 开着的面板：框（上边框写标题和在跑几条，下边框写按键提示）、框里排成的行，和每一行是后台面板里的第几条命令
/// （点哪一行点中哪一条）；没开的行是空的。框里最多 `max` 行（输入框上面剩下的，「窗口小的时候」第 1 条）。
pub fn lines(
    panel: Option<Panel>,
    board: &Board,
    config: &Config,
    width: u16,
    now: Instant,
    max: usize,
) -> (Chrome, Vec<Line<'static>>, Vec<Option<usize>>) {
    let words = &config.text.jobs;
    match panel {
        None => (Chrome::default(), Vec::new(), Vec::new()),
        Some(Panel::Background { selected, open }) => {
            let count = board.shells_running().to_string();
            let meta = vec![
                Span::styled(" · ", theme::dim()),
                Span::styled(words.active.replace("{count}", &count), theme::dim()),
            ];
            let chrome = Chrome::new(&words.title, meta).hint(&words.hint);
            let (lines, map) = background(board, selected, open, config, width, now);
            let rows = map.into_iter().zip(lines).collect();
            let (map, lines) = super::panel::fit(rows, Some(selected), max)
                .into_iter()
                .unzip();
            (chrome, lines, map)
        }
    }
}

/// 后台面板框里的：每条一行，点开的下面铺底色写输出。
fn background(
    board: &Board,
    selected: usize,
    open: Option<u64>,
    config: &Config,
    width: u16,
    now: Instant,
) -> (Vec<Line<'static>>, Vec<Option<usize>>) {
    let mut out = Vec::new();
    let mut map = Vec::new();
    for (i, job) in board.shells().into_iter().enumerate() {
        let (state, state_style) = state(job, config, now);
        let picked = i == selected;
        let style = if picked {
            Style::new().add_modifier(Modifier::BOLD)
        } else {
            theme::dim()
        };
        // 照命令列表的样子：选中的 ❯、铺强调色的底，状态贴着右边（`ui/panel.rs`）。
        let title = vec![Span::styled(job.title.clone(), style)];
        let state = Some(Span::styled(state, state_style));
        out.push(super::panel::item(picked, title, state, width));
        map.push(Some(i));
        if open == Some(job.id) {
            for line in expanded(job, config.layout.job_preview_rows, width) {
                out.push(line);
                map.push(Some(i));
            }
        }
    }
    (out, map)
}

/// 点开的一条：空行、最后几行输出（缩进两格）、空行，整块铺底色、铺满宽度。
fn expanded(job: &Job, rows: usize, width: u16) -> Vec<Line<'static>> {
    let shade = theme::shade();
    let inner = width.saturating_sub(2);
    let fill = |text: String| {
        let pad = usize::from(width).saturating_sub(text.width());
        Line::styled(format!("{text}{}", " ".repeat(pad)), shade)
    };
    let start = job.output.len().saturating_sub(rows);
    std::iter::once(fill(String::new()))
        .chain(
            job.output[start..]
                .iter()
                .map(|l| fill(format!("  {}", clip(l, inner)))),
        )
        .chain(std::iter::once(fill(String::new())))
        .collect()
}

/// 一条命令后面的状态和颜色：失败的红，别的 `faint`。
fn state(job: &Job, config: &Config, now: Instant) -> (String, Style) {
    let words = &config.text.jobs;
    let elapsed = meter::clock(job.elapsed(now).as_secs());
    match job.state {
        JobState::Running => (words.running.replace("{elapsed}", &elapsed), theme::faint()),
        JobState::Done => (words.done.replace("{elapsed}", &elapsed), theme::faint()),
        JobState::Failed(code) => (
            words.failed.replace("{code}", &code.to_string()),
            theme::error(),
        ),
        JobState::Stopped => (words.stopped.clone(), theme::faint()),
    }
}

/// 画面板：`outer` 连框，`text` 是框里放字的那一块。
pub fn draw(frame: &mut Frame, outer: Rect, text: Rect, chrome: Chrome, lines: Vec<Line<'static>>) {
    super::panel::draw(frame, outer, text, chrome, lines);
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::lines;
    use crate::app::Panel;
    use crate::config::Config;
    use crate::jobs::{Board, Feed};
    use crate::theme;

    fn board(config: &Config, t0: Instant) -> Board {
        let (mut feed, mut board) = (Feed::default(), Board::default());
        feed.start_shell(&config.fake, &mut board, t0);
        feed.start_shell(&config.fake, &mut board, t0);
        for n in 1..=3 {
            feed.advance(
                &config.fake,
                &mut board,
                t0 + Duration::from_millis(1200 * n),
            );
        }
        board.stop(board.jobs[0].id, t0 + Duration::from_secs(4));
        board
    }

    #[test]
    fn the_panel_lists_running_then_finished_with_the_selected_one_marked() {
        let config = Config::builtin().unwrap();
        let t0 = Instant::now();
        let board = board(&config, t0);
        let panel = Some(Panel::Background {
            selected: 0,
            open: None,
        });
        let (chrome, lines, map) = lines(
            panel,
            &board,
            &config,
            70,
            t0 + Duration::from_secs(12),
            usize::MAX,
        );
        let text: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
        // 2026-09-30 项目主人：三样一个框，标题嵌在上边框、按键提示嵌在下边框，框里不空行。
        let title: String = chrome.title.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(title, "后台 · 1 个在跑的命令");
        assert_eq!(
            chrome.hint.as_deref(),
            Some("↑/↓ 选 · Enter 展开 · x 停止 · Esc 关闭")
        );
        assert!(
            text[0].starts_with("❯ npm run build"),
            "在跑的在前、选中的带 ❯：{}",
            text[0]
        );
        assert!(text[0].ends_with("（运行中 12s）"));
        assert!(text[1].starts_with("  cargo test"), "结束了的在后");
        assert!(text[1].ends_with("（已停止）"));
        // 选中的铺强调色的底、字深色，状态贴着右边。
        assert_eq!(lines[0].style, theme::picked_bar());
        assert_eq!(lines[0].width(), 70);
        assert_ne!(lines[1].style, theme::picked_bar(), "没选中的不铺");
        assert_eq!(map, [Some(0), Some(1)], "点哪一行点中哪一条");
    }

    #[test]
    fn an_opened_command_shows_its_output_underneath_on_a_shade() {
        let config = Config::builtin().unwrap();
        let t0 = Instant::now();
        let board = board(&config, t0);
        let id = board.shells()[0].id;
        let panel = Some(Panel::Background {
            selected: 0,
            open: Some(id),
        });
        let (_, lines, map) = lines(
            panel,
            &board,
            &config,
            70,
            t0 + Duration::from_secs(5),
            usize::MAX,
        );
        let text: Vec<String> = lines
            .iter()
            .map(|l| l.to_string().trim_end().to_string())
            .collect();
        let out = &board.shells()[0].output;
        assert_eq!(text[1], "", "点开的下面先空一行");
        assert_eq!(text[2], format!("  {}", out[0]), "输出缩进两格");
        assert_eq!(text[1 + out.len() + 1], "");
        assert!(
            text[1 + out.len() + 2].starts_with("  cargo test"),
            "下一条接着"
        );
        assert_eq!(lines[2].style, theme::shade(), "铺底色");
        assert_eq!(lines[2].width(), 70, "铺满宽度");
        assert!(
            map[1..1 + out.len() + 2].iter().all(|m| *m == Some(0)),
            "点开的那一块都算这一条"
        );
    }
}
