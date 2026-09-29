//! 这一轮出过几件事（`transcript/beat.rs`）：运行状态行照它变没变换词。

use super::apply;
use crate::core::{Block, Push};
use crate::transcript::Transcript;

#[test]
fn every_new_thing_in_a_turn_moves_the_beat() {
    let mut t = Transcript::default();
    let mut beats = vec![t.beat()];
    let steps = [
        vec![Push::TurnStarted(1, None)],
        vec![Push::BlockStart {
            index: 0,
            block: Block::Reasoning,
        }],
        vec![Push::BlockStart {
            index: 1,
            block: Block::ToolCall("shell".into()),
        }],
        vec![
            Push::Delta {
                index: 1,
                text: "{\"command\":\"ls\"}".into(),
            },
            Push::BlockEnd(1),
            Push::Calls(vec!["c1".into()]),
            Push::ToolResult {
                call_id: "c1".into(),
                status: crate::core::ToolStatus::Ok,
                text: "a.txt".into(),
                said: None,
            },
        ],
        vec![Push::BlockStart {
            index: 0,
            block: Block::Text,
        }],
    ];
    for pushes in steps {
        apply(&mut t, pushes);
        beats.push(t.beat());
    }
    // 开一轮不算；开始想、想完（下一块开始）加开一步、工具出了结果、开始写回答，各往上走。
    assert!(beats.windows(2).skip(1).all(|w| w[1] > w[0]), "{beats:?}");
}
