//! 请求形状探针：带角色扮演提示的人格聊五轮（施工 P-1 补，`docs/blueprint/kernel/request.md`「事实」、「组装」第 6 条）。
//! 真内核照剧本跑：第 1、4 轮开头注入一块提示，排在人说的那句后面，这一轮的请求以它结尾；system 以风格锁结尾。每一次请求
//! 和存档（`docs/designs/samples/probe/reminder/`）逐字节比，查五条性质；和同一份剧本在不带提示的策略上跑的比，每一次请求
//! 只差风格锁和提示那几块：去掉它们就一字不差。

mod support;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::request::{Message, Request};
use miyu_kernel::testkit::{Line, Stage};
use support::reminder::{STYLE_LOCK, block};
use support::{check, files, matches_the_archive, sent, stage, stage_with};

/// 剧本，在替身 `s` 上跑：五轮闲聊。
fn script(mut s: Stage) -> Stage {
    for (said, reply) in [
        ("在吗", "在呢。"),
        ("今天有点累", "那就早点休息。"),
        ("嗯，晚安", "晚安。"),
        ("早上好", "早。"),
        ("吃了吗", "吃过了。"),
    ] {
        s.advance(1);
        s.model([Line::says(reply)]);
        s.say(said);
    }
    s
}

/// 一块字。
fn text(text: &str) -> Block {
    Block::Text(Text {
        text: text.to_string(),
    })
}

/// 去掉风格锁和提示那几块，交回去掉了几块。
fn plain_again(request: &Request) -> (Request, usize) {
    let mut request = request.clone();
    let lock = format!("\n\n{}", STYLE_LOCK.trim_end());
    request.system = request
        .system
        .strip_suffix(&lock)
        .expect("system 以风格锁结尾")
        .to_string();
    let reminder = text(&block());
    let mut dropped = 0;
    for message in &mut request.messages {
        if let Message::User { blocks } = message {
            let before = blocks.len();
            blocks.retain(|block| *block != reminder);
            dropped += before - blocks.len();
        }
    }
    (request, dropped)
}

#[test]
fn the_reminder_session_matches_the_archive() {
    matches_the_archive("reminder", &script(stage_with(support::reminder::policy)));
}

#[test]
fn the_same_script_gives_the_same_bytes() {
    let run = || files(&script(stage_with(support::reminder::policy)));
    assert_eq!(run(), run());
}

#[test]
fn the_reminder_session_keeps_the_properties() {
    let session = script(stage_with(support::reminder::policy));
    if let Err(why) = check(&sent(&session)) {
        panic!("{why}");
    }
    let requests = session.requests();
    assert_eq!(requests.len(), 5);
    let last_two = |index: usize| match requests[index].1.messages.last() {
        Some(Message::User { blocks }) => blocks[blocks.len().saturating_sub(2)..].to_vec(),
        other => panic!("最后一条是 user：{other:?}"),
    };
    assert_eq!(last_two(0), [text("在吗"), text(&block())]);
    assert_eq!(last_two(3), [text("早上好"), text(&block())]);
    for (index, said) in [(1, "今天有点累"), (2, "嗯，晚安"), (4, "吃了吗")] {
        assert_eq!(
            last_two(index).last(),
            Some(&text(said)),
            "第 {} 轮没有提示",
            index + 1
        );
    }
}

/// 不带提示的策略上跑同一份剧本：每一次请求只差风格锁和提示那几块，去掉就一字不差。第 1 到 3 次带着第一轮那一块，第 4、
/// 5 次再多第四轮那一块。
#[test]
fn it_differs_from_a_persona_without_one_only_by_the_lock_and_the_blocks() {
    let (now, plain) = (
        script(stage_with(support::reminder::policy)),
        script(stage()),
    );
    assert_eq!(now.requests().len(), plain.requests().len());
    let mut counts = Vec::new();
    for ((_, new), (_, old)) in now.requests().iter().zip(plain.requests()) {
        let (stripped, dropped) = plain_again(new);
        assert_eq!(stripped, *old, "去掉风格锁和提示就是不带提示的那一份");
        counts.push(dropped);
    }
    assert_eq!(counts, [1, 1, 1, 2, 2]);
}
