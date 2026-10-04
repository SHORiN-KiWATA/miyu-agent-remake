//! 子代理状态行（蓝图 `tui.md`「后台命令、子代理和侧边栏」第 6 条，设计 13 第七节，照 Claude Code）：
//! 框下面那一行下面空一行，`● 主会话`，每个子代理一行 `○ 名字  正在做什么…  token · 用时`。照主会话的任务表：
//! 在跑的，加上正在看的那个（报完了也留着，写它怎么结束的，「切进子会话」第 5 条）。正在看的那个会话实心圆，别的空心；
//! 子代理自己又派了的，名字后面暗色写 `（+N）`。焦点在这一块时，那一行前面品红的 `❯ `、字照常的颜色，别的行暗。

use std::time::Instant;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use unicode_width::UnicodeWidthStr;

use super::rows::clip;
use crate::app::App;
use crate::config::Config;
use crate::jobs::{Job, JobState};
use crate::{meter, theme};

/// 看的这一边：正在看哪个会话、每个子代理自己又派了几个在跑的、鼠标和焦点在哪一行。
#[derive(Clone, Copy)]
pub struct Tree<'a> {
    /// 正在看的子会话；看着主会话是 `None`。
    pub viewing: Option<&'a str>,
    /// 子会话自己又派了几个在跑的子代理。
    pub nested: &'a dyn Fn(&str) -> usize,
    /// 鼠标悬停的第几行（第 0 行是空行）。
    pub hover: Option<usize>,
    /// 焦点在第几行：0 是主会话，1 起是子代理（2026-09-30 项目主人：主会话那一行原来选不中，切不回去）。
    pub focus: Option<usize>,
}

/// 这一块占几行（连上面的空行）；没有列着的子代理是 0。
pub fn height(count: usize, most: usize) -> u16 {
    if count == 0 {
        return 0;
    }
    u16::try_from(2 + count.min(most) + usize::from(count > most)).unwrap_or(u16::MAX)
}

/// 每一行：空行、主会话、子代理（名字、正在做什么各对齐成一列，用时和 token 靠右）、收起来的。
pub fn lines(
    agents: &[&Job],
    config: &Config,
    width: u16,
    most: usize,
    tree: &Tree,
    now: Instant,
) -> Vec<Line<'static>> {
    let words = &config.text.jobs;
    let (hover, focus) = (tree.hover, tree.focus);
    // 第 `row` 行（第 0 行是空行）的字：焦点所在的照常的颜色；悬停的亮一档；焦点不在这一块时正在看的那一行照常；
    // 别的暗（2026-09-30 项目主人：原来焦点那一行整行品红）。
    let style = |row: usize, viewed: bool| {
        if focus == Some(row - 1) {
            Style::new()
        } else if hover == Some(row) {
            theme::hover()
        } else if viewed && focus.is_none() {
            Style::new()
        } else {
            theme::dim()
        }
    };
    // 每行前面两格：焦点所在的那一行写品红的 `❯ `。
    let lead = |row: usize, style: Style| {
        if focus == Some(row - 1) {
            Span::styled("❯ ", theme::picked())
        } else {
            Span::styled("  ", style)
        }
    };
    let dot = |on: bool| if on { "●" } else { "○" };
    let main_style = style(1, tree.viewing.is_none());
    let mut out = vec![
        Line::raw(""),
        Line::from(vec![
            lead(1, main_style),
            Span::styled(
                format!("{} {}", dot(tree.viewing.is_none()), words.main),
                main_style,
            ),
        ]),
    ];
    // 名字连着「（+N）」算宽，对齐成一列。
    let label = |job: &Job| {
        let nested = job.session.as_deref().map_or(0, |s| (tree.nested)(s));
        if nested > 0 {
            format!("{}（+{nested}）", job.title)
        } else {
            job.title.clone()
        }
    };
    let name_w = agents.iter().map(|a| label(a).width()).max().unwrap_or(0);
    for (i, job) in agents.iter().take(most).enumerate() {
        let row = i + 2;
        // token 不带「Σ」（2026-09-30 项目主人）。
        let stats = format!(
            "{} · {}",
            meter::short(job.tokens),
            meter::clock(job.elapsed(now).as_secs())
        );
        let on = tree.viewing.is_some() && job.session.as_deref() == tree.viewing;
        let title = label(job);
        let name = format!("{} {title}{}", dot(on), " ".repeat(name_w - title.width()));
        let room = usize::from(width).saturating_sub(2 + name.width() + 2 + stats.width() + 2);
        let what = if job.running() {
            job.doing.clone()
        } else {
            ended(job, config, now)
        };
        let doing = clip(&what, u16::try_from(room).unwrap_or(0));
        let pad = room.saturating_sub(doing.width()) + 2;
        let style = style(row, on);
        out.push(Line::from(vec![
            lead(row, style),
            Span::styled(format!("{name}  {doing}{}", " ".repeat(pad)), style),
            Span::styled(stats, style),
        ]));
    }
    if agents.len() > most {
        let more = words
            .more
            .replace("{count}", &(agents.len() - most).to_string());
        let style = style(most + 2, false);
        out.push(Line::from(vec![
            lead(most + 2, style),
            Span::styled(more, style),
        ]));
    }
    out
}

/// 报完了还列着的（正在看它）：正在做什么那一列写它怎么结束的，照面板的写法去掉括号；完成的只写「完成」，用时在右边。
fn ended(job: &Job, config: &Config, now: Instant) -> String {
    if job.state == JobState::Done {
        return config.text.jobs.finished.clone();
    }
    let (state, _) = super::background::state(job, config, now);
    state.trim_matches(['（', '）']).to_string()
}

/// 画这一块：照主会话的任务表（`App::listed_agents`）。
pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    if area.height == 0 {
        return;
    }
    let most = app.config.layout.agent_rows;
    let nested = |session: &str| app.nested_agents(session);
    let tree = Tree {
        viewing: app.viewing(),
        nested: &nested,
        hover: app.agents_hover,
        focus: match app.focus() {
            crate::focus::Focus::Agent(i) => Some(i),
            _ => None,
        },
    };
    let agents = app.listed_agents();
    let lines = lines(
        &agents,
        &app.config,
        area.width,
        most,
        &tree,
        Instant::now(),
    );
    frame.render_widget(Paragraph::new(lines), area);
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use ratatui::style::Style;
    use unicode_width::UnicodeWidthStr;

    use super::Tree;
    use super::{height, lines};
    use crate::config::Config;
    use crate::core::{JobEnd, JobReason, JobStart};
    use crate::jobs::Board;

    fn agent(
        board: &mut Board,
        job: &str,
        title: &str,
        session: &str,
        t0: Instant,
        doing: &str,
        tokens: u64,
    ) {
        let start = JobStart {
            call_id: String::new(),
            job: job.into(),
            agent: true,
            title: title.into(),
            session: Some(session.into()),
        };
        board.start(&start, None, t0);
        let row = board.agent_mut(session).unwrap();
        row.doing = doing.into();
        row.tokens = tokens;
    }

    fn texts(rows: &[ratatui::text::Line]) -> Vec<String> {
        rows.iter().map(|l| l.to_string()).collect()
    }

    #[test]
    fn a_blank_then_the_main_session_then_one_row_per_agent() {
        let config = Config::builtin().unwrap();
        let mut board = Board::default();
        assert_eq!(
            height(board.listed(None).len(), 5),
            0,
            "没有子代理：不占地方"
        );
        let t0 = Instant::now();
        agent(
            &mut board,
            "j1",
            "查旧版缓存",
            "s1",
            t0,
            "读 docs/fixed",
            12_400,
        );
        agent(&mut board, "j2", "找主题映射", "s2", t0, "列 themes", 800);
        let none = |_: &str| 0;
        let main = Tree {
            viewing: None,
            nested: &none,
            hover: None,
            focus: None,
        };
        let listed = board.listed(None);
        let later = t0 + Duration::from_secs(65);
        let text = texts(&lines(&listed, &config, 70, 5, &main, later));
        assert_eq!(text[0], "");
        assert_eq!(text[1], "  ● 主会话", "前面留两格给箭头");
        assert!(
            text[2].starts_with("  ○ 查旧版缓存  读 docs/fixed"),
            "{}",
            text[2]
        );
        assert!(
            text[2].ends_with("12.4k · 1m 05s"),
            "token 在用时左边：{}",
            text[2]
        );
        // 名字、正在做什么各对齐成一列，用时靠右。
        let doing = |t: &str, what: &str| t[..t.find(what).unwrap()].width();
        assert_eq!(doing(&text[2], "读"), doing(&text[3], "列"));
        assert_eq!(text[2].width(), 70);
        assert_eq!(text[3].width(), 70);
        assert_eq!(height(listed.len(), 5), 4);
        // 超过上限的收成一行。
        assert_eq!(height(listed.len(), 1), 4);
        let short = lines(&listed, &config, 70, 1, &main, t0);
        assert_eq!(short.last().unwrap().to_string(), "  还有 1 个");
    }

    #[test]
    fn the_focused_row_has_a_magenta_arrow_and_plain_text_and_the_rest_are_dim() {
        // 2026-09-30 项目主人：左边箭头品红，选中那一行照常的颜色，别的暗；主会话那一行也能选中（原来选不中，切不回去）。
        let config = Config::builtin().unwrap();
        let mut board = Board::default();
        let t0 = Instant::now();
        agent(&mut board, "j1", "查旧版缓存", "s1", t0, "", 0);
        agent(&mut board, "j2", "找主题映射", "s2", t0, "", 0);
        let none = |_: &str| 0;
        let at = |focus| Tree {
            viewing: None,
            nested: &none,
            hover: None,
            focus,
        };
        let listed = board.listed(None);
        let second = lines(&listed, &config, 70, 5, &at(Some(2)), t0);
        assert!(
            second[3].to_string().starts_with("❯ ○ 找主题映射"),
            "焦点那一行左边一个箭头"
        );
        assert_eq!(second[3].spans[0].style, crate::theme::picked(), "箭头品红");
        assert_eq!(second[3].spans[1].style, Style::new(), "字照常的颜色");
        assert!(second[2].to_string().starts_with("  ○ 查旧版缓存"));
        assert_eq!(second[2].spans[1].style, crate::theme::dim());
        assert_eq!(
            second[1].spans[1].style,
            crate::theme::dim(),
            "焦点在别处：正在看的主会话也暗"
        );
        let main = lines(&listed, &config, 70, 5, &at(Some(0)), t0);
        assert_eq!(main[1].to_string(), "❯ ● 主会话");
        assert_eq!(main[1].spans[0].style, crate::theme::picked());
        assert_eq!(main[1].spans[1].style, Style::new());
        assert_eq!(main[2].spans[1].style, crate::theme::dim());
        // 焦点不在这一块：正在看的那一行照常，别的暗。
        let idle = lines(&listed, &config, 70, 5, &at(None), t0);
        assert_eq!(idle[1].spans[1].style, Style::new());
        assert_eq!(idle[2].spans[1].style, crate::theme::dim());
    }

    #[test]
    fn the_session_being_viewed_is_the_solid_dot_and_nested_ones_are_counted() {
        // 2026-09-30 项目主人要切进子会话：正在看的那个实心圆，主会话变空心；它自己又派了的写（+N）。
        let config = Config::builtin().unwrap();
        let mut board = Board::default();
        let t0 = Instant::now();
        agent(&mut board, "j1", "查旧版缓存", "s1", t0, "", 0);
        agent(&mut board, "j2", "找主题映射", "s2", t0, "", 0);
        let nested = |s: &str| usize::from(s == "s2") * 2;
        let tree = Tree {
            viewing: Some("s2"),
            nested: &nested,
            hover: None,
            focus: None,
        };
        let text = texts(&lines(&board.listed(Some("s2")), &config, 70, 5, &tree, t0));
        assert_eq!(text[1], "  ○ 主会话");
        assert!(text[2].starts_with("  ○ 查旧版缓存"), "{}", text[2]);
        assert!(text[3].starts_with("  ● 找主题映射（+2）"), "{}", text[3]);
    }

    #[test]
    fn the_viewed_agent_stays_listed_after_it_reports() {
        // 2026-09-30 项目主人：子代理报完了底下那一块没了，切不回主会话。正在看它的留着，写它怎么结束的。
        let config = Config::builtin().unwrap();
        let mut board = Board::default();
        let t0 = Instant::now();
        agent(
            &mut board,
            "j1",
            "慢慢读",
            "s1",
            t0,
            "执行命令 · sleep 20",
            2300,
        );
        let end = JobEnd {
            job: "j1".into(),
            reason: JobReason::Finished,
            exit_code: None,
            signal: None,
            duration_ms: Some(22_000),
            text: "读完了".into(),
        };
        board.end(&end, t0 + Duration::from_secs(22));
        assert!(board.listed(None).is_empty(), "回到主会话：报完了的不列");
        let listed = board.listed(Some("s1"));
        assert_eq!(height(listed.len(), 5), 3);
        let none = |_: &str| 0;
        let tree = Tree {
            viewing: Some("s1"),
            nested: &none,
            hover: None,
            focus: None,
        };
        let text = texts(&lines(
            &listed,
            &config,
            70,
            5,
            &tree,
            t0 + Duration::from_secs(99),
        ));
        assert!(text[2].starts_with("  ● 慢慢读  完成"), "{}", text[2]);
        assert!(
            text[2].ends_with("2.3k · 22s"),
            "用时停在报完的时候：{}",
            text[2]
        );
    }
}
