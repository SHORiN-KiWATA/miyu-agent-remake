//! 重做、编辑上一句要的（蓝图 `tui.md`「斜杠命令」`/redo`、「输入框」第 13 条）：开最后一轮的那一句；核心拒了时撤掉
//! 先画上的那句。

use super::apply;
use crate::core::{Block, EndReason, Push};
use crate::transcript::{Kind, Transcript};

/// 你说一句，开第 `n` 轮，她答完。
fn turn(t: &mut Transcript, n: u64, said: &str) {
    t.user(said.into(), Vec::new());
    apply(
        t,
        vec![
            Push::UserMessage(n * 10),
            Push::TurnStarted(n, Some(n * 10)),
            Push::TurnEnded(EndReason::Completed),
        ],
    );
}

fn said(t: &Transcript) -> Option<&str> {
    t.last_said().map(|e| e.text.as_str())
}

#[test]
fn the_sentence_that_opened_the_last_turn_is_the_one_to_redo() {
    let mut t = Transcript::default();
    assert_eq!(said(&t), None, "一轮都没有");
    turn(&mut t, 1, "一");
    turn(&mut t, 2, "二");
    assert_eq!(said(&t), Some("二"));
    apply(&mut t, vec![Push::Reverted(vec![2])]);
    assert_eq!(said(&t), Some("一"), "撤掉的不算");
    // 最后一轮不是你说的话开的（回报叫醒的）：不能重做。
    apply(
        &mut t,
        vec![
            Push::TurnStarted(3, None),
            Push::BlockStart {
                index: 0,
                block: Block::Text,
            },
            Push::Delta {
                index: 0,
                text: "后台命令做完了".into(),
            },
            Push::BlockEnd(0),
            Push::TurnEnded(EndReason::Completed),
        ],
    );
    assert_eq!(said(&t), None);
}

#[test]
fn a_refused_redo_takes_back_the_line_drawn_ahead() {
    let mut t = Transcript::default();
    turn(&mut t, 1, "一");
    t.user("一（改过）".into(), Vec::new());
    t.drop_unsent();
    let users: Vec<&str> = t
        .entries
        .iter()
        .filter(|e| e.kind == Kind::User)
        .map(|e| e.text.as_str())
        .collect();
    assert_eq!(users, ["一"], "落了盘的不动");
}

#[test]
fn the_redone_turn_hides_at_once_and_comes_back_if_refused() {
    // 2026-09-30 项目主人：/redo、/edit 发出去时，重发的那句先在底下闪一下，旧的一轮才消失。发出去就藏，
    // 核心拒了再显示回来。
    let mut t = Transcript::default();
    turn(&mut t, 1, "一");
    turn(&mut t, 2, "二");
    assert_eq!(t.hide_last_turn(), Some(2));
    let shown: Vec<&str> = t
        .entries
        .iter()
        .filter(|e| e.kind == Kind::User && !e.hidden)
        .map(|e| e.text.as_str())
        .collect();
    assert_eq!(shown, ["一"], "第 2 轮先藏起来");
    t.show_turn(2);
    assert_eq!(said(&t), Some("二"), "拒了的显示回来");
}
