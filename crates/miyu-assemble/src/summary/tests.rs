//! 取摘要的测试：有收尾的、没收尾的、只有草稿以外的、空的；思考不算；指令并进最后一条 user 或者另起一条。

use miyu_kernel::block::{Block, Reasoning, Text};
use miyu_kernel::request::Message;

use super::*;

fn text(text: &str) -> Block {
    Block::Text(Text {
        text: text.to_string(),
    })
}

fn thinking(text: &str) -> Block {
    Block::Reasoning(Reasoning {
        text: text.to_string(),
        private: None,
    })
}

#[test]
fn the_summary_between_its_tags_is_taken() {
    let reply = [text(
        "<analysis>\nThe user asked for X.\n</analysis>\n\n<summary>\n1. Primary Request: X.\n</summary>\n",
    )];
    assert_eq!(extract(&reply).as_deref(), Some("1. Primary Request: X."));
}

#[test]
fn a_summary_cut_off_by_the_output_limit_runs_to_the_end() {
    let reply = [text(
        "<analysis>a</analysis><summary>\n1. Primary Request: X.\n2. Key",
    )];
    assert_eq!(
        extract(&reply).as_deref(),
        Some("1. Primary Request: X.\n2. Key")
    );
}

#[test]
fn without_summary_tags_the_draft_is_dropped_and_the_rest_kept() {
    let reply = [text("Before.\n<analysis>draft</analysis>\nThe summary.\n")];
    assert_eq!(extract(&reply).as_deref(), Some("Before.\n\nThe summary."));
    // 草稿没收尾：到末尾都是草稿。
    assert_eq!(
        extract(&[text("Kept.<analysis>draft")]).as_deref(),
        Some("Kept.")
    );
    // 什么标签都没有：整段就是摘要。
    assert_eq!(extract(&[text("  Plain.  ")]).as_deref(), Some("Plain."));
}

#[test]
fn nothing_usable_is_none_and_thinking_does_not_count() {
    assert_eq!(extract(&[]), None);
    assert_eq!(extract(&[text("  \n ")]), None);
    assert_eq!(extract(&[text("<analysis>only a draft</analysis>")]), None);
    assert_eq!(extract(&[text("<summary> </summary>")]), None);
    assert_eq!(extract(&[thinking("<summary>S</summary>")]), None);
    // 几块正文连起来读：标签跨块也认。
    let reply = [thinking("hmm"), text("<summ"), text("ary>S</summary>")];
    assert_eq!(extract(&reply).as_deref(), Some("S"));
}

#[test]
fn the_instruction_joins_a_trailing_user_message_or_starts_one() {
    let mut messages = vec![Message::User {
        blocks: vec![text("hi")],
    }];
    instruct(&mut messages, "<summarize/>");
    assert_eq!(
        messages,
        [Message::User {
            blocks: vec![text("hi"), text("<summarize/>")],
        }]
    );
    let mut messages = vec![Message::Assistant {
        blocks: vec![text("好。")],
    }];
    instruct(&mut messages, "<summarize/>");
    assert_eq!(messages.len(), 2);
    assert_eq!(
        messages[1],
        Message::User {
            blocks: vec![text("<summarize/>")],
        }
    );
    let mut messages = Vec::new();
    instruct(&mut messages, "<summarize/>");
    assert_eq!(messages.len(), 1);
}
