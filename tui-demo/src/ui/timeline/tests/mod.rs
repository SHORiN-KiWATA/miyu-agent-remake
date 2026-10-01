//! 时间线排成的行（蓝图 `tui.md`「时间线」）：一步一行带图标、步间连接线、收起那一行带用时、只有一步的点一下就铺开。

use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::rows;
use crate::core::ToolStatus;
use crate::theme;
use crate::transcript::{Segment, Step, StepKind, ToolState};
use crate::ui::rows::Row;
use crate::ui::test_support::Fixture;

mod language;

/// 一步：从 `start` 秒开始，用了 `took` 秒。
fn step(kind: StepKind, t0: Instant, start: u64, took: u64) -> Step {
    let mut s = Step::new(kind);
    s.started = t0 + Duration::from_secs(start);
    s.took = Some(Duration::from_secs(took));
    s
}

fn thought(t0: Instant, start: u64) -> Step {
    step(
        StepKind::Thought {
            text: "先看看目录".into(),
        },
        t0,
        start,
        1,
    )
}

fn command(t0: Instant, start: u64, status: ToolStatus) -> Step {
    step(
        StepKind::Tool {
            name: "shell".into(),
            args: String::new(),
            parsed: json!({"command": "ls", "description": "列目录"}),
            state: ToolState::Done(status),
            output: "a.txt".into(),
            said: None,
        },
        t0,
        start,
        1,
    )
}

fn segment(steps: Vec<Step>, open: Option<bool>) -> Segment {
    let mut s = Segment::new();
    s.steps = steps;
    s.finished = true;
    s.open = open;
    s
}

fn text(rows: &[Row]) -> Vec<String> {
    rows.iter()
        .map(|r| r.line.to_string().trim_end().to_string())
        .collect()
}

#[test]
fn a_folded_segment_is_one_line_ending_with_its_time() {
    let f = Fixture::new();
    let t0 = Instant::now();
    let seg = segment(
        vec![
            thought(t0, 0),
            command(t0, 1, ToolStatus::Ok),
            command(t0, 2, ToolStatus::Ok),
        ],
        None,
    );
    assert_eq!(
        text(&rows(0, &seg, &f.ctx())),
        ["  Ran 2 commands · 1 thought · 3s"]
    );
}

#[test]
fn a_lone_command_names_the_folded_line_and_turns_red_when_it_failed() {
    let f = Fixture::new();
    let t0 = Instant::now();
    let seg = segment(vec![thought(t0, 0), command(t0, 1, ToolStatus::Ok)], None);
    assert_eq!(
        text(&rows(0, &seg, &f.ctx())),
        ["  列目录 · 1 thought · 2s"]
    );
    let failed = segment(
        vec![thought(t0, 0), command(t0, 1, ToolStatus::Error)],
        None,
    );
    let row = &rows(0, &failed, &f.ctx())[0];
    assert!(
        row.line
            .to_string()
            .contains("列目录 · 1 thought · 1 err · 2s")
    );
    assert_eq!(row.line.spans.last().unwrap().style, theme::error());
}

#[test]
fn an_open_segment_lists_its_steps_with_icons_and_connectors() {
    let f = Fixture::new();
    let icons = &f.config.icons;
    let t0 = Instant::now();
    let seg = segment(
        vec![
            thought(t0, 0),
            command(t0, 1, ToolStatus::Ok),
            command(t0, 2, ToolStatus::Error),
        ],
        Some(true),
    );
    let rows = rows(0, &seg, &f.ctx());
    let shell = icons.tool("shell");
    assert_eq!(
        text(&rows),
        [
            "  Ran 2 commands · 1 thought · 1 err · 3s".to_string(),
            "  │".into(),
            format!("  {} 已思考 · 1.0s", icons.think),
            "  │ 先看看目录".into(),
            "  │".into(),
            format!("  {shell} shell · 列目录"),
            "  │ ls".into(),
            "  │".into(),
            format!("  {} shell · 列目录", icons.error),
            "  │ ls".into(),
        ]
    );
    // 出错的那一步图标换成叉、字红，竖线一路红到下一步的标题（`tui.md`「时间线」第 12 条）。
    let preview = &rows[9].line.spans;
    assert!(
        preview
            .iter()
            .any(|s| s.content == "│ " && s.style == theme::error())
    );
    assert!(
        preview
            .iter()
            .any(|s| s.content == "ls" && s.style == theme::error())
    );
    // 没出错的那一步下面的连接线照旧暗。
    assert!(rows[7].line.spans.iter().all(|s| s.style != theme::error()));
}

#[test]
fn the_rail_stays_red_down_to_the_next_step_after_a_failure() {
    let f = Fixture::new();
    let t0 = Instant::now();
    let seg = segment(
        vec![
            thought(t0, 0),
            command(t0, 1, ToolStatus::Error),
            thought(t0, 2),
        ],
        Some(true),
    );
    let rows = rows(0, &seg, &f.ctx());
    let lines = text(&rows);
    // 出错那一步：标题、预览，接着的连接线，再是下一步的标题。
    let after = lines.iter().position(|l| l == "  │ ls").unwrap() + 1;
    assert_eq!(lines[after], "  │");
    assert!(
        rows[after]
            .line
            .spans
            .iter()
            .any(|s| s.content.contains('│') && s.style == theme::error()),
        "出错那一步下面的连接线红"
    );
}

#[test]
fn every_line_of_a_command_preview_has_the_rail() {
    let f = Fixture::new();
    let t0 = Instant::now();
    let mut two = command(t0, 1, ToolStatus::Ok);
    if let StepKind::Tool { parsed, .. } = &mut two.kind {
        *parsed = json!({"command": "cd src\nls", "description": "列目录"});
    }
    let seg = segment(vec![thought(t0, 0), two], Some(true));
    let lines = text(&rows(0, &seg, &f.ctx()));
    assert_eq!(&lines[lines.len() - 2..], ["  │ cd src", "  │ ls"]);
}

#[test]
fn a_one_step_segment_opens_straight_into_its_content() {
    let f = Fixture::new();
    let t0 = Instant::now();
    let seg = segment(vec![thought(t0, 0)], Some(true));
    let rows = rows(0, &seg, &f.ctx());
    assert_eq!(text(&rows), ["  Thought for 1s", "", "    先看看目录", ""]);
    assert!(rows.iter().all(|r| r.shade), "连收起那一行一起铺底色");
}

#[test]
fn a_running_segment_has_no_head_and_a_command_without_text_has_no_preview() {
    let f = Fixture::new();
    let t0 = Instant::now();
    let mut running = segment(vec![thought(t0, 0)], None);
    running.finished = false;
    running.steps.push(Step::new(StepKind::Tool {
        name: "shell".into(),
        args: "{\"comm".into(),
        parsed: Value::Null,
        state: ToolState::Preparing,
        output: String::new(),
        said: None,
    }));
    let lines = text(&rows(0, &running, &f.ctx()));
    // 进行中的不画打头那一行；参数还在流、命令还没有字，只有「准备」那一行，不画一根空的竖线。
    let icons = &f.config.icons;
    assert_eq!(
        lines,
        [
            format!("  {} 已思考 · 1.0s", icons.think),
            "  │ 先看看目录".into(),
            "  │".into(),
            format!("⠋ {} 准备shell", icons.tool("shell")),
        ]
    );
}

/// 一件还没结果的 shell：`state` 是准备中或者在跑。
fn pending(state: ToolState) -> Step {
    Step::new(StepKind::Tool {
        name: "shell".into(),
        args: String::new(),
        parsed: json!({"command": "sleep 3", "description": "等一会"}),
        state,
        output: String::new(),
        said: None,
    })
}

/// 转圈的行：槽里是转圈的帧。
fn spinning(rows: &[Row], spinner: &[String]) -> usize {
    rows.iter()
        .filter(|r| {
            let line = r.line.to_string();
            spinner.iter().any(|f| line.starts_with(f.as_str()))
        })
        .count()
}

#[test]
fn only_one_step_spins_at_a_time() {
    let f = Fixture::new();
    let spinner = &f.config.timeline.spinner;
    let t0 = Instant::now();
    // 她一次发两件：第一件参数写完了，第二件还在写。只转还在写的那一件（`tui.md`「时间线」第 19 条）。
    let mut seg = segment(
        vec![pending(ToolState::Running), pending(ToolState::Preparing)],
        None,
    );
    seg.finished = false;
    let got = rows(0, &seg, &f.ctx());
    assert_eq!(spinning(&got, spinner), 1, "{:?}", text(&got));
    let turning: Vec<String> = text(&got)
        .into_iter()
        .filter(|l| spinner.iter().any(|f| l.starts_with(f.as_str())))
        .collect();
    assert!(
        turning[0].contains("准备"),
        "转的是还在写的那一件：{turning:?}"
    );
    // 都写完了、等结果：转最前面那一件；有了结果的不转，后面等着的也不转。
    let mut seg = segment(
        vec![
            command(t0, 0, ToolStatus::Ok),
            pending(ToolState::Running),
            pending(ToolState::Running),
        ],
        None,
    );
    seg.finished = false;
    let got = rows(0, &seg, &f.ctx());
    assert_eq!(spinning(&got, spinner), 1, "{:?}", text(&got));
    assert_eq!(seg.active(), Some(1));
    // 排着队、还没开始的：槽里一个暗色的 `·`（2026-09-29 项目主人）。
    let queued: Vec<&Row> = got
        .iter()
        .filter(|r| r.line.to_string().starts_with("· "))
        .collect();
    assert_eq!(queued.len(), 1, "只有后面等着的那一件：{:?}", text(&got));
    let mark = queued[0]
        .line
        .spans
        .iter()
        .find(|s| s.content == "· ")
        .unwrap();
    assert_eq!(mark.style, theme::dim());
}

#[test]
fn an_empty_thought_has_only_its_title() {
    // 还一个字都没有的思考不画空的竖线（2026-09-29 项目主人）。
    let f = Fixture::new();
    for text in ["", "  \n "] {
        let mut seg = segment(Vec::new(), None);
        seg.finished = false;
        seg.steps
            .push(Step::new(StepKind::Thought { text: text.into() }));
        let got = self::text(&rows(0, &seg, &f.ctx()));
        assert_eq!(got.len(), 1, "只有标题：{got:?}");
        assert!(got[0].contains("思考中"));
    }
}

#[test]
fn a_thought_scrolls_its_last_lines_and_keeps_them_when_done() {
    let f = Fixture::new();
    let tl = &f.config.timeline;
    let text: String = (1..=20).map(|i| format!("第 {i} 行\n")).collect();
    assert_eq!(tl.thought_rows, 15, "思考的预览 15 行，和命令的分开");
    let mut running = segment(Vec::new(), None);
    running.finished = false;
    running
        .steps
        .push(Step::new(StepKind::Thought { text: text.clone() }));
    let live = self::text(&rows(0, &running, &f.ctx()));
    assert!(
        live[0].contains("思考中") && !live[0].contains("第"),
        "标题不带一瞥：{}",
        live[0]
    );
    assert_eq!(live.len(), 1 + tl.thought_rows, "下面滚着最后几行");
    assert_eq!(live.last().unwrap(), "  │ 第 20 行");
    // 想完：预览不收起，还是最后几行。
    let t0 = Instant::now();
    let done = step(StepKind::Thought { text }, t0, 0, 2);
    let seg = segment(vec![done, command(t0, 2, ToolStatus::Ok)], Some(true));
    let lines = self::text(&rows(0, &seg, &f.ctx()));
    let at = lines.iter().position(|l| l.contains("已思考")).unwrap();
    assert_eq!(lines[at + tl.thought_rows], "  │ 第 20 行", "{lines:?}");
    assert_eq!(lines[at + 1], "  │ 第 6 行");
}

#[test]
fn a_folded_thought_only_segment_is_just_its_time() {
    let f = Fixture::new();
    let t0 = Instant::now();
    let done = step(
        StepKind::Thought {
            text: "先看看目录\n最后决定只改连接线".into(),
        },
        t0,
        0,
        2,
    );
    let only = segment(vec![done], None);
    assert_eq!(
        text(&rows(0, &only, &f.ctx())),
        ["  Thought for 2s"],
        "不接思考的字"
    );
}

#[test]
fn hovering_a_step_lifts_it_one_notch_and_leaves_the_rail_dim() {
    use crate::ui::rows::Target;
    let f = Fixture::new();
    let t0 = Instant::now();
    let seg = segment(
        vec![thought(t0, 0), command(t0, 1, ToolStatus::Ok)],
        Some(true),
    );
    let mut ctx = f.ctx();
    ctx.hover = Some(Target::Step(0, 1));
    let rows = rows(0, &seg, &ctx);
    // 第 5 行是命令的标题，第 6 行是它的预览（`tui.md`「时间线」第 6 条）。
    let title = rows[5].line.spans.last().unwrap();
    assert_eq!(title.style, theme::hover(), "亮一档，不是原色");
    assert_ne!(theme::hover(), ratatui::style::Style::new());
    let preview = &rows[6].line.spans;
    assert!(
        preview
            .iter()
            .any(|s| s.content == "│ " && s.style == theme::dim()),
        "竖线不跟着亮"
    );
    assert!(
        preview
            .iter()
            .any(|s| s.content == "ls" && s.style == theme::hover())
    );
}

fn edit(t0: Instant, start: u64) -> Step {
    step(
        StepKind::Tool {
            name: "edit".into(),
            args: String::new(),
            parsed: json!({"path": "src/a.rs", "edits": [{"old_string": "a\nb", "new_string": "a\nc\nd"}]}),
            state: ToolState::Done(ToolStatus::Ok),
            output: String::new(),
            said: None,
        },
        t0,
        start,
        1,
    )
}

#[test]
fn a_refused_edit_is_an_error_and_adds_no_lines() {
    // 2026-09-30 项目主人：只读时写入被拒，和写成了看起来一模一样。被拒的算出错，不算进编辑、不算加减的行数。
    let f = Fixture::new();
    let t0 = Instant::now();
    let mut refused = edit(t0, 1);
    if let StepKind::Tool { state, .. } = &mut refused.kind {
        *state = ToolState::Done(ToolStatus::Denied);
    }
    assert!(refused.failed(), "被拒的算出错：图标换成叉、整行红");
    let seg = segment(vec![edit(t0, 0), refused.clone()], None);
    let got = rows(0, &seg, &f.ctx());
    assert_eq!(text(&got), ["  Used 1 tool · 1 edit +2 -1 · 1 err · 2s"]);
    let err = got[0]
        .line
        .spans
        .iter()
        .find(|s| s.content == "1 err")
        .unwrap();
    assert_eq!(err.style, theme::error(), "出错的那一格红");
    // 只想了一下、写入被拒：照样写出来，不收成 `Thought for`（真模型实测：原来试过写入这件事都看不见了）。
    let seg = segment(vec![thought(t0, 0), refused], None);
    assert_eq!(
        text(&rows(0, &seg, &f.ctx())),
        ["  Used 1 tool · 1 thought · 1 err · 2s"]
    );
}

#[test]
fn a_folded_edit_carries_its_added_and_removed_lines() {
    let f = Fixture::new();
    let t0 = Instant::now();
    let seg = segment(vec![thought(t0, 0), edit(t0, 1)], None);
    let rows = rows(0, &seg, &f.ctx());
    assert_eq!(text(&rows), ["  Made 1 edit +2 -1 · 1 thought · 2s"]);
    let spans = &rows[0].line.spans;
    assert!(
        spans
            .iter()
            .any(|s| s.content == "+2" && s.style == theme::added())
    );
    assert!(
        spans
            .iter()
            .any(|s| s.content == "-1" && s.style == theme::removed())
    );
}

#[test]
fn folding_and_opening_follow_the_config_until_someone_clicks() {
    let mut f = Fixture::new();
    let t0 = Instant::now();
    // 不收起：做完的一段照样展开。
    f.config.timeline.fold = false;
    let seg = segment(vec![thought(t0, 0), command(t0, 1, ToolStatus::Ok)], None);
    let lines = text(&rows(0, &seg, &f.ctx()));
    assert!(
        lines.iter().any(|l| l.contains("shell · 列目录")),
        "{lines:?}"
    );
    // 命令默认铺开全文：命令、空行、输出（照点开的样子）。
    f.config.timeline.expand.command = true;
    let lines = text(&rows(0, &seg, &f.ctx()));
    assert!(
        lines.iter().any(|l| l.trim() == "a.txt"),
        "铺开了输出：{lines:?}"
    );
    // 人亲手收起的照人点的。
    let mut seg = seg;
    seg.steps[1].open = Some(false);
    let lines = text(&rows(0, &seg, &f.ctx()));
    assert!(!lines.iter().any(|l| l.trim() == "a.txt"), "{lines:?}");
    // 人亲手收起整段的，也照人点的。
    seg.open = Some(false);
    assert_eq!(rows(0, &seg, &f.ctx()).len(), 1);
}

mod agent;
mod icons;
mod live;
mod narrow;
mod session_arg;
mod waiting;
