//! `/copy` 复制的那一段（蓝图 `tui.md`「斜杠命令」`/copy`）。

use super::apply;
use crate::core::{Block, Push};
use crate::transcript::Transcript;

#[test]
fn copy_takes_her_last_turns_reply_joined_and_skips_undone_ones() {
    // 2026-09-30 项目主人要的 /copy：她上一轮的回答，Markdown 原文，被工具隔成几段的连起来，撤掉的不算。
    let mut t = Transcript::default();
    assert_eq!(t.last_reply(), None, "还没有回答");
    let reply = |t: &mut Transcript, turn: u64, parts: &[&str]| {
        let mut pushes = vec![Push::TurnStarted(turn, None)];
        for (i, text) in parts.iter().enumerate() {
            let index = u64::try_from(i).unwrap() * 2;
            pushes.push(Push::BlockStart {
                index,
                block: Block::Text,
            });
            pushes.push(Push::Delta {
                index,
                text: (*text).into(),
            });
            pushes.push(Push::BlockEnd(index));
            pushes.push(Push::BlockStart {
                index: index + 1,
                block: Block::ToolCall("read".into()),
            });
            pushes.push(Push::BlockEnd(index + 1));
        }
        pushes.push(Push::TurnEnded(crate::core::EndReason::Completed));
        apply(t, pushes);
    };
    reply(&mut t, 1, &["第一轮"]);
    reply(&mut t, 2, &["**先看看**", "看完了：`a.rs` 没问题"]);
    assert_eq!(
        t.last_reply().as_deref(),
        Some("**先看看**\n\n看完了：`a.rs` 没问题")
    );
    apply(&mut t, vec![Push::Reverted(vec![2])]);
    assert_eq!(t.last_reply().as_deref(), Some("第一轮"), "撤掉的不算");
}
