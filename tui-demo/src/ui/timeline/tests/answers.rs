//! 「提问」那一步下面接着回答（蓝图 `tui.md`「确认和提问的抽屉」第 6 条，2026-10-11 项目主人：和别的步一样的竖线，
//! 一题一行「问题：回答」）。

use std::time::Instant;

use serde_json::json;

use super::{rows, segment, step, text};
use crate::core::ToolStatus;
use crate::transcript::{StepKind, ToolState};
use crate::ui::test_support::Fixture;

#[test]
fn the_answers_sit_under_the_question_step_on_its_rail() {
    let mut f = Fixture::new();
    f.config.timeline.fold = false;
    let t0 = Instant::now();
    let mut asked = step(
        StepKind::Tool {
            name: "ask_user".into(),
            args: String::new(),
            parsed: json!({}),
            state: ToolState::Done(ToolStatus::Ok),
            output: "answers".into(),
            said: None,
        },
        t0,
        0,
        1,
    );
    asked.answers = vec!["接下来做什么：只是看看".into()];
    let lines = text(&rows(0, &segment(vec![asked], None), &f.ctx()));
    assert!(
        lines.iter().any(|l| l.trim() == "│ 接下来做什么：只是看看"),
        "{lines:?}"
    );
}
