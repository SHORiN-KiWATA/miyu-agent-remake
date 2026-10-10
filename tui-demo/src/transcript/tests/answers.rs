//! 提问的回答写进时间线里「提问」那一步（蓝图 `tui.md`「确认和提问的抽屉」第 6 条，2026-10-11 项目主人：不另起引用块）。

use super::apply;
use crate::core::{Block, Push};
use crate::transcript::Transcript;

#[test]
fn answers_hang_on_the_step_that_asked() {
    let mut t = Transcript::default();
    apply(
        &mut t,
        vec![
            Push::TurnStarted(1, None),
            Push::BlockStart {
                index: 0,
                block: Block::ToolCall("ask_user".into()),
            },
            Push::Delta {
                index: 0,
                text: "{}".into(),
            },
            Push::BlockEnd(0),
            Push::Calls(vec!["q1".into()]),
        ],
    );
    assert!(t.answered("q1", vec!["接下来做什么：看看".into()]));
    let step = &t.entries[0].segment.as_ref().unwrap().steps[0];
    assert_eq!(step.answers, ["接下来做什么：看看"]);
    assert!(
        !t.answered("nope", vec!["x".into()]),
        "找不到那一步的交回 false，由调用的一方另写"
    );
}
