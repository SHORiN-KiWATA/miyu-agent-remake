//! NapCat 没成算成哪一句（施工 O-31，`onebot.md` 第一条「平台工具（一）」第 5 条）：回了失败的照原话（`message`，空的换
//! `wording`，再空的写 `retcode`），去掉首尾空白、截到 200 个字符；等不到、断了各是一句。

use serde_json::json;

use super::{Undone, undone};
use crate::onebot::CallError;

/// 回了失败、原话照 `reply` 的。
fn failed(reply: serde_json::Value) -> Undone {
    undone(CallError::Failed(reply))
}

#[test]
fn a_refusal_carries_what_napcat_said() {
    let said = |text: &str| Undone::Failed("failed", Some(text.to_string()));
    assert_eq!(
        failed(
            json!({"status": "failed", "retcode": 200, "message": "  cannot ban owner ", "wording": "x"})
        ),
        said("cannot ban owner")
    );
    assert_eq!(
        failed(
            json!({"status": "failed", "retcode": 200, "message": " ", "wording": " 没有权限 "})
        ),
        said("没有权限")
    );
    assert_eq!(
        failed(json!({"status": "failed", "retcode": 1200, "message": ""})),
        said("1200")
    );
    let long = "撤".repeat(240);
    assert_eq!(
        failed(json!({"status": "failed", "message": long})),
        said(&"撤".repeat(200))
    );
}

#[test]
fn no_answer_and_a_dropped_link_are_their_own_sentences() {
    assert_eq!(
        undone(CallError::Timeout),
        Undone::Failed("unanswered", None)
    );
    assert_eq!(
        undone(CallError::Closed),
        Undone::Failed("unreachable", None)
    );
}
