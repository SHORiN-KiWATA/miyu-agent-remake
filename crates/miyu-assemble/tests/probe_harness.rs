//! 请求形状探针：有别的 harness 来话的会话（施工 7-10，`docs/blueprint/kernel/request.md`「别的 harness 发来的话」）。
//! 真内核照剧本跑：人问一句；Claude Code 闲着时说一句，开了第二轮；人再问，她读文件的时候 Claude Code 又说一句，排在那一步的
//! 工具结果后面。每一次请求和存档（`docs/designs/samples/probe/harness/`）逐字节比，查五条性质；和同一份剧本里换成人说的
//! 比，每一次请求只多标签那两段。

use crate::support::{check, files, matches_the_archive, sent, stage};
use miyu_kernel::block::{Block, Text};
use miyu_kernel::id::CommandId;
use miyu_kernel::request::{Message, Request};
use miyu_kernel::testkit::{Line, Play, Stage};

/// 标签的开头：名字是 `claude-code`。
const OPEN: &str = "<agent-message from=\"claude-code\">\n";
/// 标签的收尾。
const CLOSE: &str = "</agent-message>\n";

/// 剧本：Claude Code 的两句照 `said` 说出来（它自己说，或者换成人说）。
fn script(said: fn(&mut Stage, &str) -> CommandId) -> Stage {
    let mut s = stage();
    // 1. 人问一句，她答。
    s.model([Line::says("好，我看看。")]);
    s.say("CI 为什么红了？");
    // 2. 她闲着，Claude Code 说一句：由它开一轮，回合开始的地方就是它。
    s.advance(2);
    s.model([Line::says("知道了，那我不用查了。")]);
    said(&mut s, "CI 修好了：macOS 上的临时目录换成了真实路径。");
    // 3. 人再问，她读文件；读的时候 Claude Code 又说一句：下一次请求里排在这一步的工具结果后面。
    s.advance(2);
    s.model([
        Line::calls(
            "我读一下测试。",
            &[("read", r#"{"path":"tests/support/mod.rs"}"#)],
        ),
        Line::says("测试也过了。"),
    ]);
    s.tools([Play::done("fn temp_dir() -> PathBuf { canonical() }").held()]);
    s.say("那测试呢？");
    let reading = s.ran().last().expect("派了读").0;
    said(&mut s, "测试我跑过了，都过了。");
    s.release_tool(reading);
    s
}

/// Claude Code 说的。
fn harness() -> Stage {
    script(|s, words| s.harness_says("claude-code", words))
}

/// 同一份剧本，换成人说的。
fn person() -> Stage {
    script(|s, words| s.say(words))
}

/// 去掉标签：带标签的那一块换回原话。
fn untagged(request: &Request) -> Request {
    let mut request = request.clone();
    for message in &mut request.messages {
        let Message::User { blocks } = message else {
            continue;
        };
        for block in blocks {
            if let Block::Text(Text { text }) = block
                && let Some(inside) = text
                    .strip_prefix(OPEN)
                    .and_then(|rest| rest.strip_suffix(CLOSE))
            {
                *text = inside.strip_suffix('\n').unwrap_or(inside).to_string();
            }
        }
    }
    request
}

/// 最后一条 user 的最后一块字。
fn last_text(request: &Request) -> Option<&str> {
    match request.messages.last() {
        Some(Message::User { blocks }) => match blocks.last() {
            Some(Block::Text(text)) => Some(text.text.as_str()),
            _ => None,
        },
        _ => None,
    }
}

#[test]
fn the_harness_session_matches_the_archive() {
    matches_the_archive("harness", &harness());
}

#[test]
fn the_same_script_gives_the_same_bytes() {
    assert_eq!(files(&harness()), files(&harness()));
}

#[test]
fn the_harness_session_keeps_the_properties() {
    let session = harness();
    if let Err(why) = check(&sent(&session)) {
        panic!("{why}");
    }
    let requests = session.requests();
    assert_eq!(requests.len(), 4);
    assert_eq!(
        last_text(&requests[1].1),
        Some(format!("{OPEN}CI 修好了：macOS 上的临时目录换成了真实路径。\n{CLOSE}").as_str()),
        "它开的那一轮，最后一块是它那一句，带着标签"
    );
    let busy = &requests[3].1;
    assert!(
        matches!(busy.messages.last(), Some(Message::User { blocks })
            if matches!(blocks.as_slice(), [Block::Text(Text { text })] if text.starts_with(OPEN))),
        "回合中途到的，单独一条 user 排在工具结果后面：{:?}",
        busy.messages.last()
    );
    assert!(matches!(
        busy.messages.iter().rev().nth(1),
        Some(Message::Tool { .. })
    ));
}

#[test]
fn it_differs_from_the_persons_words_only_by_the_tags() {
    let (harness, person) = (harness(), person());
    assert_eq!(harness.requests().len(), person.requests().len());
    let mut tagged = 0;
    for ((_, from), (_, said)) in harness.requests().iter().zip(person.requests()) {
        tagged += usize::from(from != said);
        assert_eq!(untagged(from), *said, "去掉标签就是人说的那一份");
    }
    assert_eq!(tagged, 3, "第 2 次起每一次都带着它的话");
}
