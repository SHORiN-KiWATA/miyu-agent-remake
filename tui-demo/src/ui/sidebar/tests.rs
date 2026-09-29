//! 侧边栏（蓝图 `tui.md`「后台命令、子代理和侧边栏」第 4、7 条）：分栏、待办的收法、会话那几段的写法。

use std::time::{Duration, Instant};

use ratatui::layout::Rect;
use ratatui::style::Modifier;

use super::{split, todo_lines};
use crate::config::Config;
use crate::jobs::{Board, Feed, Todo, TodoState};

#[test]
fn only_wide_windows_get_a_sidebar() {
    let layout = Config::builtin().unwrap().layout;
    let (main, side) = split(Rect::new(0, 0, 119, 40), &layout);
    assert_eq!((main.width, side.width), (119, 0));
    let (main, side) = split(Rect::new(0, 0, 160, 40), &layout);
    assert_eq!(side.width, layout.sidebar_width);
    assert_eq!(main.width + 1 + side.width, 160, "中间一根竖线");
    assert_eq!(side.right(), 160);
}

#[test]
fn the_full_list_marks_each_state_and_wraps_under_the_text() {
    let config = Config::builtin().unwrap();
    let (mut feed, mut board) = (Feed::default(), Board::default());
    assert!(todo_lines(&board, &config, 30, 5, false).is_empty());
    let t0 = Instant::now();
    feed.start_todo(&config.fake, &mut board, t0);
    feed.advance(
        &config.fake,
        &mut board,
        t0 + Duration::from_millis(config.fake.todos.every_ms),
    );
    let lines = todo_lines(&board, &config, 12, 5, true);
    let text: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    assert_eq!(text[0], format!("待办 1/{}", config.fake.todos.items.len()));
    assert!(text[1].starts_with("☑ 读蓝图"));
    assert!(
        lines[1].spans[1]
            .style
            .add_modifier
            .contains(Modifier::CROSSED_OUT),
        "做完的划掉"
    );
    let active = text.iter().position(|t| t.starts_with("■ ")).unwrap();
    assert!(text[active + 1].starts_with("  "), "折下来的和字对齐");
    assert!(text.iter().any(|t| t.starts_with("☐ ")));
}

#[test]
fn a_finished_list_goes_away() {
    let config = Config::builtin().unwrap();
    let (mut feed, mut board) = (Feed::default(), Board::default());
    let t0 = Instant::now();
    feed.start_todo(&config.fake, &mut board, t0);
    let every = config.fake.todos.every_ms;
    let n = config.fake.todos.items.len() as u64;
    for k in 1..n {
        feed.advance(
            &config.fake,
            &mut board,
            t0 + Duration::from_millis(every * k),
        );
    }
    assert!(
        !todo_lines(&board, &config, 30, 5, false).is_empty(),
        "还剩一项：还在"
    );
    feed.advance(
        &config.fake,
        &mut board,
        t0 + Duration::from_millis(every * n),
    );
    assert_eq!(board.todo_progress(), Some((n as usize, n as usize)));
    assert!(
        todo_lines(&board, &config, 30, 5, false).is_empty(),
        "全做完了：收掉"
    );
}

fn long_board(done: usize, pending: usize) -> Board {
    let mut board = Board::default();
    let state = |i: usize| match i.cmp(&done) {
        std::cmp::Ordering::Less => TodoState::Done,
        std::cmp::Ordering::Equal => TodoState::Active,
        std::cmp::Ordering::Greater => TodoState::Pending,
    };
    board.todos = (0..=done + pending)
        .map(|i| Todo {
            text: format!("第 {i} 项，改时间线的连接线，让出错的那一步一路红到下一步的标题"),
            state: state(i),
        })
        .collect();
    board
}

#[test]
fn a_long_list_shows_only_what_is_in_progress() {
    let config = Config::builtin().unwrap();
    // 20 项：做完 12，在做 1，还有 7。
    let board = long_board(12, 7);
    let lines = todo_lines(&board, &config, 30, 5, false);
    let text: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    assert_eq!(text.len(), 6, "标题加 5 行：{text:?}");
    assert_eq!(text[0], "待办 12/20");
    assert_eq!(text[1], "☑ 做完 12 项", "做完的收成一行");
    assert!(text[2].starts_with("■ 第 12 项"), "接着是在做的那项");
    assert!(text[2].ends_with('…'), "长的截掉加 …");
    assert!(text[3].starts_with("☐ 第 13 项"));
    assert!(text[4].starts_with("☐ 第 14 项"));
    assert_eq!(text[5], "… 还有 5 项");
    // 点开：全部，长的折行。
    let full = todo_lines(&board, &config, 30, 5, true);
    assert!(full.len() > 21, "全部 20 项、还折行");
}

#[test]
fn a_short_list_shows_every_item() {
    let config = Config::builtin().unwrap();
    let board = long_board(1, 3);
    let text: Vec<String> = todo_lines(&board, &config, 30, 5, false)
        .iter()
        .map(|l| l.to_string())
        .collect();
    assert_eq!(text.len(), 6, "5 项放得下：一项一行");
    assert!(text[1].starts_with("☑ 第 0 项"));
    assert!(text[2].starts_with("■ 第 1 项"));
}

#[test]
fn the_sidebar_is_laid_out_in_sections() {
    use crate::transcript::Transcript;
    use ratatui::style::Modifier;
    let config = Config::builtin().unwrap();
    let mut t = Transcript::default();
    t.session = Some("019a5c3e-8f2b-7c61-9e0d-2f5a7b3c1d4e".into());
    let cwd = "~/Documents/github/miyu-agent-remake/.worktrees/proto-tui-demo/tui-demo";
    // 还没用过：会话、工作目录两段；名字当标题，没起名字的暗色写「未命名会话」，下面是短编号。
    let (lines, id_row) = super::info_lines(&t, &config, 36, cwd);
    let text: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    assert_eq!(
        text,
        [
            "未命名会话",
            "  #019a5c3e",
            "",
            "工作目录",
            "  ~/Documents/github/",
            "  miyu-agent-remake/.worktrees/",
            "  proto-tui-demo/tui-demo",
        ]
    );
    assert_eq!(id_row, Some(1), "点编号那一行复制完整编号");
    let bold = |l: &ratatui::text::Line| l.style.add_modifier.contains(Modifier::BOLD);
    assert!(!bold(&lines[0]), "不加粗");
    assert_eq!(
        lines[0].style,
        crate::theme::side_title(),
        "没起名字的也是蓝色的标题"
    );
    assert_eq!(lines[3].style, crate::theme::side_title());
    assert_eq!(lines[1].style, crate::theme::dim(), "内容暗");
    assert!(
        !lines[3].style.add_modifier.contains(Modifier::BOLD),
        "小标题不加粗"
    );
    // 起了名字、用过了：接上下文（带进度条）、用量。
    t.title = Some("整理 src 目录".into());
    t.model = Some(("deepseek-flash".into(), "deepseek".into()));
    t.context = 200_000;
    t.total.output = 3500;
    let (lines, _) = super::info_lines(&t, &config, 36, "~/src");
    let text: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    assert_eq!(text[0], "整理 src 目录");
    assert_eq!(
        lines[0].style,
        crate::theme::side_title(),
        "起了名字的标题一样蓝、不加粗"
    );
    let at = text.iter().position(|l| l == "上下文").unwrap();
    assert_eq!(text[at + 1], "  200k / 1M · 20%");
    let bar = &text[at + 2];
    let width = config.layout.bar.width;
    assert_eq!(bar.chars().count(), 2 + width, "进度条 {width} 格，不铺满");
    assert_eq!(
        bar.chars().filter(|c| *c == '▰').count(),
        width / 5,
        "占了两成"
    );
    // 内容一律暗：上下文、用量的每一行。
    let at = text.iter().position(|l| l == "上下文").unwrap();
    assert_eq!(lines[at + 1].style, crate::theme::dim());
    let at = text.iter().position(|l| l == "用量").unwrap();
    assert_eq!(text[at + 1], "  共 3.5k token");
    assert_eq!(text[at + 2], "  输入 0 · 输出 3.5k");
    assert_eq!(text[at + 3], "  缓存命中 0%", "带 %");
}

#[test]
fn compactions_and_cache_breaks_show_only_once_they_happen() {
    use crate::transcript::Transcript;
    let config = Config::builtin().unwrap();
    let mut t = Transcript::default();
    t.model = Some(("deepseek-flash".into(), "deepseek".into()));
    t.context = 200_000;
    t.total.output = 3500;
    let text = |t: &Transcript| -> Vec<String> {
        let (lines, _) = super::info_lines(t, &config, 36, "~/src");
        lines.iter().map(|l| l.to_string()).collect()
    };
    let before = text(&t);
    assert!(
        !before
            .iter()
            .any(|l| l.contains("压缩") || l.contains("断裂")),
        "没压过、没断过：都不写"
    );
    t.cache.compactions = 2;
    t.cache.breaks = 1;
    let after = text(&t);
    let at = after.iter().position(|l| l == "上下文").unwrap();
    assert_eq!(after[at + 3], "  压缩 2 次", "在进度条下面");
    let at = after.iter().position(|l| l == "用量").unwrap();
    assert_eq!(after[at + 4], "  缓存断裂 1 次", "在命中率下面");
}

#[test]
fn a_section_title_is_blue_and_its_lines_indented() {
    use ratatui::style::Modifier;
    use ratatui::text::Line;
    let out = super::section(vec![Line::raw("待办 1/9"), Line::raw("■ 写测试")]);
    assert_eq!(out[0].to_string(), "待办 1/9");
    assert_eq!(out[0].style, crate::theme::side_title());
    assert!(
        !out[0].style.add_modifier.contains(Modifier::BOLD),
        "不加粗"
    );
    assert_eq!(out[1].to_string(), "  ■ 写测试");
}

#[test]
fn a_long_directory_breaks_after_a_slash_and_never_leaves_one_alone() {
    let lines = super::wrap_path(
        "/tmp/claude-1000/-home-shorin-Documents-github-Miyu/fbb4de14-613a-4306-bb73-84c0bd837ed0/scratchpad/work",
        34,
    );
    assert_eq!(
        lines,
        [
            "/tmp/claude-1000/",
            "-home-shorin-Documents-github-Miyu",
            "/fbb4de14-613a-4306-bb73-84c0bd837",
            "ed0/scratchpad/work",
        ]
    );
    assert!(lines.iter().all(|l| l != "/"), "不留一个孤零零的 /");
}

#[test]
fn a_little_context_still_lights_one_cell() {
    let marks = crate::config::Config::builtin().unwrap().layout.bar;
    let bar = super::bar(1500, 1_000_000, 34, &marks).to_string();
    assert_eq!(
        bar.chars().filter(|c| *c == '▰').count(),
        1,
        "用过就至少亮一格"
    );
    let empty = super::bar(0, 1_000_000, 34, &marks).to_string();
    assert_eq!(empty.chars().filter(|c| *c == '▰').count(), 0);
}
