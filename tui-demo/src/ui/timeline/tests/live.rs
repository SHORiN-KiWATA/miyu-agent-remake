//! 进行中的那一段封顶（蓝图 `tui.md`「时间线」第 20 条）。

use std::time::Instant;

use super::{command, rows, segment, text};
use crate::core::ToolStatus;
use crate::transcript::Step;
use crate::ui::test_support::Fixture;
use crate::ui::timeline::{live_cap, release_on_fold};

#[test]
fn a_running_segment_shows_only_its_latest_rows() {
    // 进行中的那一段只露最新的那些行，上面的直接不画，不加提示、不能点开（`tui.md`「时间线」第 20 条）。
    let f = Fixture::new();
    let tl = &f.config.timeline;
    let t0 = Instant::now();
    let steps: Vec<Step> = (0..12).map(|k| command(t0, k, ToolStatus::Ok)).collect();
    let mut seg = segment(steps, None);
    seg.finished = false;
    let shown = rows(0, &seg, &f.ctx());
    assert_eq!(shown.len(), live_cap(tl), "最多露这么多行");
    let mut whole = seg.clone();
    whole.open = Some(true);
    let all = rows(0, &whole, &f.ctx());
    assert!(all.len() > live_cap(tl), "人点开的不受限");
    assert_eq!(
        text(&shown),
        text(&all[all.len() - live_cap(tl)..]),
        "露的是最新的"
    );
    assert!(
        !text(&shown).iter().any(|l| l.contains("上面还有")),
        "不加提示"
    );
    // 做完的、人点开的：不受限；短的照旧。
    whole.finished = true;
    assert!(rows(0, &whole, &f.ctx()).len() > live_cap(tl));
    let mut short = segment(vec![command(t0, 0, ToolStatus::Ok)], None);
    short.finished = false;
    assert_eq!(rows(0, &short, &f.ctx()).len(), 2, "标题和预览");
}

#[test]
fn the_cap_fits_one_whole_thought() {
    // 封顶的高度刚好放得下一步完整的思考：几条命令之后在想，思考的标题和它露的每一行都看得见。
    use crate::transcript::StepKind;
    let f = Fixture::new();
    let tl = &f.config.timeline;
    assert_eq!(live_cap(tl), tl.thought_rows + 1, "预览加标题");
    let t0 = Instant::now();
    let mut steps: Vec<Step> = (0..8).map(|k| command(t0, k, ToolStatus::Ok)).collect();
    let long: String = (1..=40).map(|i| format!("第 {i} 行\n")).collect();
    steps.push(Step::new(StepKind::Thought { text: long }));
    let mut seg = segment(steps, None);
    seg.finished = false;
    let shown = text(&rows(0, &seg, &f.ctx()));
    assert_eq!(shown.len(), live_cap(tl));
    assert!(
        shown.iter().any(|l| l.contains("思考中")),
        "思考的标题看得见：{shown:?}"
    );
    assert_eq!(shown.last().unwrap(), "  │ 第 40 行");
}

#[test]
fn the_cap_follows_a_taller_command_preview() {
    // 命令预览调得比思考预览还大：封顶跟着变，一步完整的命令（标题、预览、已省略那一行）放得下。
    use crate::transcript::{StepKind, ToolState};
    let mut f = Fixture::new();
    f.config.timeline.preview_rows = 20;
    let tl = &f.config.timeline;
    assert_eq!(live_cap(tl), 20 + 1 + 1, "命令预览、已省略那一行、标题");
    let t0 = Instant::now();
    let mut steps: Vec<Step> = (0..8).map(|k| command(t0, k, ToolStatus::Ok)).collect();
    let script: String = (1..=30).map(|i| format!("echo {i}\n")).collect();
    steps.push(Step::new(StepKind::Tool {
        name: "shell".into(),
        args: String::new(),
        parsed: serde_json::json!({"command": script, "description": "长脚本"}),
        state: ToolState::Running,
        output: String::new(),
        said: None,
    }));
    let mut seg = segment(steps, None);
    seg.finished = false;
    let shown = text(&rows(0, &seg, &f.ctx()));
    assert_eq!(shown.len(), live_cap(tl));
    assert!(
        shown.iter().any(|l| l.contains("长脚本")),
        "命令的标题看得见：{shown:?}"
    );
    assert!(
        shown.last().unwrap().contains("已省略"),
        "已省略那一行在最底下：{shown:?}"
    );
}

#[test]
fn the_switch_turns_the_limit_off_and_decides_the_viewport() {
    // 「限制工具时间线滚动区域」关掉：进行中全部展开，收起时放开视口；开着时不封顶以外的都照旧，收起不放开。
    let mut f = Fixture::new();
    assert!(f.config.timeline.limit_live, "默认开");
    assert!(
        !release_on_fold(&f.config.timeline),
        "开着：收起不放开视口，上面的不落回来"
    );
    f.config.timeline.limit_live = false;
    assert!(release_on_fold(&f.config.timeline));
    let t0 = Instant::now();
    let steps: Vec<Step> = (0..12).map(|k| command(t0, k, ToolStatus::Ok)).collect();
    let mut seg = segment(steps, None);
    seg.finished = false;
    assert!(
        rows(0, &seg, &f.ctx()).len() > live_cap(&f.config.timeline),
        "关掉不封顶"
    );
}

#[test]
fn steps_in_a_limited_running_segment_cannot_be_opened() {
    // 开着封顶时，进行中那一段里的步点不开：只露十几行，点开了多半看不全（2026-09-29 项目主人）。
    let mut f = Fixture::new();
    let t0 = Instant::now();
    let steps = vec![
        command(t0, 0, ToolStatus::Ok),
        command(t0, 1, ToolStatus::Ok),
    ];
    let mut seg = segment(steps, None);
    seg.finished = false;
    assert!(
        rows(0, &seg, &f.ctx()).iter().all(|r| r.target.is_none()),
        "进行中：点不开"
    );
    seg.finished = true;
    assert!(
        rows(0, &seg, &f.ctx()).iter().any(|r| r.target.is_some()),
        "做完了：照常点"
    );
    seg.finished = false;
    f.config.timeline.limit_live = false;
    assert!(
        rows(0, &seg, &f.ctx()).iter().any(|r| r.target.is_some()),
        "关掉封顶：照常点"
    );
}
