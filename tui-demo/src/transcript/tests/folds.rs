//! 一段收起了几次（蓝图 `tui.md`「正文」第 1 条）：界面照它在收起的那一刻放开一次视口。

use super::super::Transcript;
use super::apply;
use crate::core::{Block, Push};

#[test]
fn a_segment_counts_as_folded_the_moment_she_starts_talking() {
    let mut t = Transcript::default();
    apply(
        &mut t,
        vec![
            Push::TurnStarted(1, None),
            Push::BlockStart {
                index: 0,
                block: Block::Reasoning,
            },
            Push::Delta {
                index: 0,
                text: "想".into(),
            },
        ],
    );
    assert_eq!(t.folds(), 0, "还在想：没收起");
    apply(
        &mut t,
        vec![Push::BlockStart {
            index: 1,
            block: Block::Text,
        }],
    );
    assert_eq!(t.folds(), 1, "她一开口，那一段收起");
}
