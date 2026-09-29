//! 在等她的第一个字（蓝图 `tui.md`「时间线」第 19 条）：一轮开始、她还一块都没出。

use super::super::Transcript;
use super::apply;
use crate::core::{Block, EndReason, Push, ToolStatus};

#[test]
fn only_the_wait_for_the_first_word_counts() {
    let mut t = Transcript::default();
    assert!(!t.waiting(), "没在跑");
    apply(&mut t, vec![Push::TurnStarted(1, None)]);
    assert!(t.waiting(), "发出去了，第一个字还没来");
    apply(
        &mut t,
        vec![Push::CallFailed {
            class: "network".into(),
            message: "reset".into(),
        }],
    );
    assert!(t.waiting(), "出错等重试：这一轮还没出过字");
    apply(
        &mut t,
        vec![
            Push::BlockStart {
                index: 0,
                block: Block::ToolCall("shell".into()),
            },
            Push::BlockEnd(0),
            Push::Calls(vec!["c1".into()]),
            Push::Sent {
                seen: 3,
                changed: false,
            },
        ],
    );
    assert!(!t.waiting(), "出字了");
    apply(
        &mut t,
        vec![Push::ToolResult {
            call_id: "c1".into(),
            status: ToolStatus::Ok,
            text: "a.txt".into(),
            said: None,
        }],
    );
    assert!(!t.waiting(), "步与步之间等她：不算");
    apply(&mut t, vec![Push::TurnEnded(EndReason::Completed)]);
    assert!(!t.waiting(), "一轮结束");
    apply(&mut t, vec![Push::TurnStarted(2, None)]);
    assert!(t.waiting(), "下一轮又从头等");
}
