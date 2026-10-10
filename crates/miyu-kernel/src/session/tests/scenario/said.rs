//! 场景：回合开始交给挂接点的 `said`（施工 R-8，`docs/blueprint/kernel/session.md` 的 `RunTurnStartHooks`，`memory.md`
//! 第四条）：人开的一轮是触发它的那句话的字（几块字照先后用换行连、去掉前后空白）；只有附件的、别的 harness 说的、别的会话
//! 说的、回报开的都没有；排着的几句一起开的一轮只取触发的那一句（最后一句，有意的：同回合索引）。

use super::reports::{CHILD, dispatched};
use super::*;
use crate::block::Image;
use crate::event::ChildReason;
use crate::id::{ContentHash, MediaType};

fn text(words: &str) -> Block {
    Block::Text(Text {
        text: words.to_string(),
    })
}

fn picture() -> Block {
    Block::Image(Image {
        blob: ContentHash::of(b"png"),
        name: None,
        media_type: MediaType::parse("image/png").unwrap(),
        width: 10,
        height: 10,
        path: None,
    })
}

#[test]
fn a_persons_words_are_handed_to_the_hooks() {
    let mut s = stage();
    s.model([
        Line::says("好。"),
        Line::says("嗯。"),
        Line::says("看到了。"),
    ]);
    s.say("  我养了一只猫  ");
    s.send(vec![text("第一段"), picture(), text(" 第二段\n")]);
    s.send(vec![picture()]);
    assert_eq!(
        s.saids(),
        [
            Some("我养了一只猫".to_string()),
            Some("第一段\n 第二段".to_string()),
            None,
        ],
        "去掉前后空白；只有附件的没有"
    );
}

#[test]
fn a_harness_a_peer_and_a_report_hand_nothing() {
    let mut s = dispatched(stage());
    s.model([
        Line::says("看到了。"),
        Line::says("好。"),
        Line::says("收到。"),
    ]);
    s.child_reports(1, CHILD, ChildReason::Done, "CI 红在 macOS。");
    s.harness_says("claude-code", "CI 修好了。");
    s.peer_says("01a0d78c-ca52-7d19-8b64-0e3f5a7c2d99", "我这边好了。");
    assert_eq!(
        s.saids(),
        [Some("查一下 CI，顺便跑测试".to_string()), None, None, None],
        "派活的那一轮是人开的；回报、harness、别的会话开的都没有"
    );
}

#[test]
fn queued_words_hand_only_the_one_that_triggers() {
    let mut s = stage();
    s.model([Line::says("好").held(), Line::says("嗯。")]);
    s.say("hi");
    s.say("我养了一只猫");
    s.say("叫团子");
    s.release_model();
    assert_eq!(
        s.saids(),
        [Some("hi".to_string()), Some("叫团子".to_string())],
        "排着的两句一起开下一轮，只取触发的那一句"
    );
}
