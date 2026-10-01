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
use crate::jobs::clean::clean;
use crate::jobs::{Board, Job, JobState, PanelItem};
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
        None
        | Some(Panel::Help { .. } | Panel::Language { .. } | Panel::Sessions | Panel::Models) => {
            (Chrome::default(), Vec::new(), Vec::new())
        }
        Some(Panel::Background {
            selected,
            open,
            all,
        }) => {
            let count = board.shells_running().to_string();
            let meta = vec![
                Span::styled(" · ", theme::dim()),
                Span::styled(words.active.replace("{count}", &count), theme::dim()),
            ];
            let chrome = Chrome::new(&words.title, meta).hint(&words.hint);
            let (lines, map) = background(board, (selected, open, all), config, width, now);
            let rows = map.into_iter().zip(lines).collect();
            let (map, lines) = super::panel::fit(rows, Some(selected), max)
                .into_iter()
                .unzip();
            (chrome, lines, map)
        }
    }
}

/// 后台面板框里的：每条一行，点开的下面铺底色写输出；结束了的多于一条时收起来的那一行（第 3 条）。`panel` 是选中
/// 第几行、点开了哪一条、收起来的展开了没有。
fn background(
    board: &Board,
    (selected, open, all): (usize, Option<u64>, bool),
    config: &Config,
    width: u16,
    now: Instant,
) -> (Vec<Line<'static>>, Vec<Option<usize>>) {
    let words = &config.text.jobs;
    let mut out = Vec::new();
    let mut map = Vec::new();
    for (i, item) in board.panel_items(all).into_iter().enumerate() {
        let picked = i == selected;
        let style = if picked {
            Style::new().add_modifier(Modifier::BOLD)
        } else {
            theme::dim()
        };
        let job = match item {
            PanelItem::Job(id) => board.jobs.iter().find(|j| j.id == id),
            _ => None,
        };
        let (title, state) = match (item, job) {
            (PanelItem::More(count), _) => (
                words.more_ended.replace("{count}", &count.to_string()),
                None,
            ),
            (PanelItem::Less, _) => (words.less_ended.clone(), None),
            (_, Some(job)) => {
                let (state, state_style) = state(job, config, now);
                (job.headline(), Some(Span::styled(state, state_style)))
            }
            _ => continue,
        };
        // 照命令列表的样子：选中的 ❯、铺强调色的底，状态贴着右边（`ui/panel.rs`）。
        out.push(super::panel::item(
            picked,
            vec![Span::styled(title, style)],
            state,
            width,
        ));
        map.push(Some(i));
        if let Some(job) = job.filter(|j| open == Some(j.id)) {
            for line in expanded(job, config, now, width) {
                out.push(line);
                map.push(Some(i));
            }
        }
    }
    (out, map)
}

/// 点开的一条：空行、状态、完整的命令、空行、最后几行输出（缩进两格，照常的颜色；在跑的跟着新输出走）、空行，整块铺底色、
/// 铺满宽度。
/// 还没读到的只有状态；一个字都没有的写「没有输出」（「后台命令、子代理和侧边栏」第 3 条）。
fn expanded(job: &Job, config: &Config, now: Instant, width: u16) -> Vec<Line<'static>> {
    let shade = theme::shade();
    let fill = |text: String, style: Style| {
        let pad = usize::from(width).saturating_sub(text.width());
        Line::styled(format!("{text}{}", " ".repeat(pad)), style.patch(shade))
    };
    let (state, style) = state(job, config, now);
    // 展开的字比收着的那一行亮一档：状态暗（失败的照旧红），输出照常的颜色（2026-09-30 项目主人：原来太暗）。
    let style = if style == theme::error() {
        style
    } else {
        theme::dim()
    };
    let mut out = vec![
        fill(String::new(), Style::new()),
        fill(format!("  {state}"), style),
    ];
    // 完整的命令（那一行只写得下第一行、截掉的；2026-09-30 项目主人：展开看不到完整命令），空一行接输出。
    let room = width.saturating_sub(2);
    for (piece, _) in crate::input::pieces(job.title.trim(), room.max(1)) {
        out.push(fill(format!("  {piece}"), Style::new()));
    }
    if let Some(output) = &job.output {
        out.push(fill(String::new(), Style::new()));
        let look = &config.layout.output;
        let text = clean(&output.text, look.tab);
        let lines: Vec<&str> = text.trim_end().lines().collect();
        if lines.is_empty() {
            out.push(fill(
                format!("  {}", config.text.jobs.no_output),
                theme::dim(),
            ));
        }
        for line in &lines[lines.len().saturating_sub(look.panel_rows)..] {
            out.push(fill(format!("  {}", clip(line, room)), Style::new()));
        }
    }
    out.push(fill(String::new(), Style::new()));
    out
}

/// 一条命令后面的状态和颜色：失败的红，别的 `faint`。
pub(super) fn state(job: &Job, config: &Config, now: Instant) -> (String, Style) {
    let words = &config.text.jobs;
    let elapsed = meter::clock(job.elapsed(now).as_secs());
    match job.state {
        JobState::Running => (words.running.replace("{elapsed}", &elapsed), theme::faint()),
        JobState::Done => (words.done.replace("{elapsed}", &elapsed), theme::faint()),
        JobState::Failed(code) => (
            words.failed.replace("{code}", &code.to_string()),
            theme::error(),
        ),
        JobState::Killed(signal) => (
            words.killed.replace("{signal}", &signal.to_string()),
            theme::error(),
        ),
        JobState::Stopped => (words.stopped.clone(), theme::faint()),
        JobState::Undone => (words.undone.clone(), theme::faint()),
        JobState::Restarted => (words.restarted.clone(), theme::faint()),
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
    use crate::core::{JobEnd, JobOutput, JobReason, JobStart};
    use crate::jobs::{Board, PanelItem};
    use crate::theme;

    /// 两条后台命令：`npm run build` 在跑，`cargo test` 被停掉了。
    fn board(_config: &Config, t0: Instant) -> Board {
        let mut board = Board::default();
        let start = |job: &str| JobStart {
            call_id: String::new(),
            job: job.into(),
            agent: false,
            title: String::new(),
            session: None,
        };
        board.start(&start("j1"), Some("cargo test".into()), t0);
        board.start(&start("j2"), Some("npm run build".into()), t0);
        let stopped = JobEnd {
            job: "j1".into(),
            reason: JobReason::Stopped,
            exit_code: None,
            signal: None,
            duration_ms: Some(4000),
            text: String::new(),
        };
        board.end(&stopped, t0 + Duration::from_secs(4));
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
            all: false,
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
    fn an_opened_command_shows_its_state_and_the_last_lines_of_its_output() {
        // 施工 7-4 补的 `job.output`（2026-09-30 项目主人定现在接）：点开写状态（暗）和最后几行输出（照常的颜色，洗掉转义序列）。
        let config = Config::builtin().unwrap();
        let t0 = Instant::now();
        let mut board = board(&config, t0);
        let id = board.by_job_mut("j2").unwrap().id;
        let open = |board: &Board| {
            let panel = Some(Panel::Background {
                selected: 0,
                open: Some(id),
                all: false,
            });
            let later = t0 + Duration::from_secs(5);
            let (_, lines, map) = lines(panel, board, &config, 70, later, usize::MAX);
            let text: Vec<String> = lines
                .iter()
                .map(|l| l.to_string().trim_end().to_string())
                .collect();
            (lines, text, map)
        };
        // 还没读到：状态和完整的命令。
        let (_, text, _) = open(&board);
        assert_eq!(text[1..5], ["", "  （运行中 5s）", "  npm run build", ""]);
        let rows = config.layout.output.panel_rows;
        let many: String = (1..=rows + 3)
            .map(|i| format!("\x1b[32mline {i}\x1b[0m\n"))
            .collect();
        board.by_job_mut("j2").unwrap().output = Some(JobOutput {
            text: many,
            lines: 15,
            running: true,
            truncated: false,
        });
        let (lines, text, map) = open(&board);
        assert_eq!(text[1], "", "点开的下面先空一行");
        assert_eq!(text[2], "  （运行中 5s）");
        assert_eq!(
            lines[2].style.fg,
            theme::dim().fg,
            "状态暗，比收着的那一行亮一档"
        );
        assert_eq!(text[3], "  npm run build", "完整的命令");
        assert_eq!(text[4], "", "空一行接输出");
        assert_eq!(text[5], "  line 4", "只露最后几行");
        assert_eq!(text[4 + rows], format!("  line {}", rows + 3));
        assert_eq!(lines[5].style.fg, None, "输出照常的颜色");
        assert_eq!(text[5 + rows], "");
        assert!(text[6 + rows].starts_with("  cargo test"), "下一条接着");
        assert_eq!(lines[5].style.bg, theme::shade().bg, "铺底色");
        assert_eq!(lines[5].width(), 70, "铺满宽度");
        assert!(
            map[1..6 + rows].iter().all(|m| *m == Some(0)),
            "点开的那一块都算这一条"
        );
        // 一个字都没有：写「没有输出」。
        board.by_job_mut("j2").unwrap().output = Some(JobOutput::default());
        assert_eq!(open(&board).1[5], "  没有输出");
    }

    #[test]
    fn more_than_one_ended_command_folds_into_one_row() {
        // 2026-09-30 项目主人定：结束了的多于一条时只露最近结束的那条，别的收成一行，选中按 Enter 展开、再按收起。
        let config = Config::builtin().unwrap();
        let t0 = Instant::now();
        let mut board = board(&config, t0);
        let start = JobStart {
            call_id: String::new(),
            job: "j3".into(),
            agent: false,
            title: String::new(),
            session: None,
        };
        board.start(&start, Some("ls".into()), t0);
        let done = JobEnd {
            job: "j3".into(),
            reason: JobReason::Finished,
            exit_code: Some(0),
            signal: None,
            duration_ms: Some(9000),
            text: String::new(),
        };
        board.end(&done, t0 + Duration::from_secs(9));
        let show = |all: bool| {
            let panel = Some(Panel::Background {
                selected: 0,
                open: None,
                all,
            });
            let (_, lines, _) = lines(panel, &board, &config, 70, t0, usize::MAX);
            lines
                .iter()
                .map(|l| l.to_string().trim_end().to_string())
                .collect::<Vec<_>>()
        };
        let folded = show(false);
        assert_eq!(folded.len(), 3);
        assert!(folded[0].starts_with("❯ npm run build"));
        assert!(
            folded[1].starts_with("  ls"),
            "最近结束的那条：{}",
            folded[1]
        );
        assert_eq!(folded[2], "  已收起 1 个", "2026-09-30 项目主人改的写法");
        let all = show(true);
        assert!(all[1].starts_with("  cargo test"), "展开：照开始的先后");
        assert!(all[2].starts_with("  ls"));
        assert_eq!(all[3], "  收起已结束的");
        assert_eq!(
            board.panel_items(false),
            [PanelItem::Job(2), PanelItem::Job(3), PanelItem::More(1)]
        );
    }
}
