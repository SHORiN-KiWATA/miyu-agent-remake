//! 请求形状探针：有别的会话来话、有空了的通知的会话（施工 C-2、C-6，`docs/blueprint/kernel/request.md`「别的会话发来的话」
//! 「空了的通知」）。真内核照剧本跑：人问一句；别的会话（短编号 `22334455`）闲着时说一句，开了第二轮；人再问，她读文件的
//! 时候它又说一句，排在那一步的工具结果后面；人让她订另一个会话（短编号 `9f03b21c`）「空了告诉我」，她订了；那个会话空下来，
//! 通知开了一轮。每一次请求和存档（`docs/designs/samples/probe/peers/`）逐字节比，查五条性质；和同一份剧本里换成人说的比，
//! 每一次请求只多标签那两段（通知两边都有）。

mod support;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::id::CommandId;
use miyu_kernel::request::{Message, Request};
use miyu_kernel::testkit::{Line, Play, Stage};
use support::{check, files, matches_the_archive, sent, stage};

/// 发话的会话。
const PEER: &str = "0192f3a0-1111-7abc-8def-001122334455";
/// 标签的开头：短编号是 `22334455`。
const OPEN: &str = "<session-message from=\"22334455\">\n";
/// 标签的收尾。
const CLOSE: &str = "</session-message>\n";
/// 等的会话：短编号 `9f03b21c`（施工 C-6）。
const WATCHED: &str = "0192f3a0-2222-7abc-8def-55669f03b21c";
/// 它交来的通知那一块。
const NOTICE: &str = "<session-idle session=\"9f03b21c\" reason=\"idle\">\n测试全过了，迁移那边没问题。\n</session-idle>\n";

/// 剧本：别的会话的两句照 `said` 说出来（它自己说，或者换成人说）。
fn script(said: fn(&mut Stage, &str) -> CommandId) -> Stage {
    let mut s = stage();
    // 1. 人问一句，她答。
    s.model([Line::says("好，我看看。")]);
    s.say("CI 为什么红了？");
    // 2. 她闲着，别的会话说一句：由它开一轮，回合开始的地方就是它。
    s.advance(2);
    s.model([Line::says("知道了，那我这边接导出。")]);
    said(&mut s, "迁移写完了，按会话分区。你那边的导出可以接上了。");
    // 3. 人再问，她读文件；读的时候它又说一句：下一次请求里排在这一步的工具结果后面。
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
    said(&mut s, "迁移的测试我跑过了，都过了。");
    s.release_tool(reading);
    // 4. 人让她订另一个会话「空了告诉我」：订是一次调用，结果报 `peer.watch`（施工 C-6）。
    s.advance(2);
    s.model([
        Line::calls("我订一下。", &[("read", r#"{"path":"notify"}"#)]),
        Line::says("订好了，它跑完会告诉我。"),
    ]);
    s.tools([Play::watches(WATCHED)]);
    s.say("让 9f03b21c 跑完测试告诉我");
    // 5. 那个会话空下来：通知开一轮，回合开始的地方就是它。
    s.advance(5);
    s.model([Line::says("它跑完了，测试全过。")]);
    s.peer_idle(WATCHED, Some("测试全过了，迁移那边没问题。"));
    s
}

/// 别的会话说的。
fn peer() -> Stage {
    script(|s, words| s.peer_says(PEER, words))
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
fn the_peer_session_matches_the_archive() {
    matches_the_archive("peers", &peer());
}

#[test]
fn the_same_script_gives_the_same_bytes() {
    assert_eq!(files(&peer()), files(&peer()));
}

#[test]
fn the_peer_session_keeps_the_properties() {
    let session = peer();
    if let Err(why) = check(&sent(&session)) {
        panic!("{why}");
    }
    let requests = session.requests();
    assert_eq!(requests.len(), 7);
    assert_eq!(
        last_text(&requests[1].1),
        Some(format!("{OPEN}迁移写完了，按会话分区。你那边的导出可以接上了。\n{CLOSE}").as_str()),
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
    assert_eq!(
        last_text(&requests[6].1),
        Some(NOTICE),
        "通知开的那一轮，最后一块是通知"
    );
}

#[test]
fn it_differs_from_the_persons_words_only_by_the_tags() {
    let (peer, person) = (peer(), person());
    assert_eq!(peer.requests().len(), person.requests().len());
    let mut tagged = 0;
    for ((_, from), (_, said)) in peer.requests().iter().zip(person.requests()) {
        tagged += usize::from(from != said);
        assert_eq!(untagged(from), *said, "去掉标签就是人说的那一份");
    }
    assert_eq!(tagged, 6, "第 2 次起每一次都带着它的话");
}
