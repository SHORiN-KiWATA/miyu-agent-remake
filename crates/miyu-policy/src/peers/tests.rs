//! 别的会话发来的话进快照（施工 C-2）：出厂的快照带着防刷屏的数和标签的两份，渲染出带短编号的标签；标签坏了照名字报；
//! 以前造的快照里没有数的照出厂的数，没有标签的那种话和人的话一字不差，读进来再写出去一字不差。

use miyu_kernel::event::Event;
use miyu_kernel::history::History;
use miyu_kernel::session::Peers;

use crate::snapshot::{BuildError, Snapshot};
use crate::test_support::engineer;

/// 发话的会话：短编号 `22334455`。
const PEER: &str = r#"{"kind":"session","id":"0192f3a0-1111-7abc-8def-001122334455"}"#;
const ALICE: &str = r#"{"kind":"person","account":"alice"}"#;

/// 出厂的数。
const FACTORY: Peers = Peers {
    burst: 5,
    window: 600,
    unread: 50,
};

/// `by` 开了第一轮的日志。
fn history(by: &str) -> History {
    let lines = [
        r#"{"seq":1,"at":"2026-09-25T07:00:00.000Z","kind":"session.created","by":{"kind":"kernel"},"body":{"owner":"alice","venue":"local","policy":"sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","permission":{"level":"workspace","read_only":false}}}"#.to_string(),
        format!(
            r#"{{"seq":2,"at":"2026-09-25T07:00:00.000Z","kind":"message.user","by":{by},"cause":"c1","body":{{"blocks":[{{"type":"text","text":"迁移写完了。"}}]}}}}"#
        ),
        r#"{"seq":3,"at":"2026-09-25T07:00:00.000Z","kind":"turn.started","turn":3,"by":{"kind":"kernel"},"cause":"c1","body":{"trigger":2}}"#.to_string(),
    ];
    let mut history = History::default();
    for line in lines {
        history.append(Event::from_line(&line).unwrap());
    }
    history
}

/// 照快照组装这段日志，请求的字节。
fn rendered(snapshot: &Snapshot, by: &str) -> String {
    let request = snapshot.policy().unwrap().assembler.assemble(&history(by));
    String::from_utf8(request.canonical_bytes()).unwrap()
}

/// 去掉快照字节里从 `from` 起、到 `to` 为止（含）的那一格。
fn without(text: &str, from: &str, to: &str) -> String {
    let start = text.find(from).unwrap();
    let end = start + text[start..].find(to).unwrap() + to.len();
    text[..start].to_string() + &text[end..]
}

#[test]
fn the_numbers_and_the_tags_go_in() {
    let snapshot = engineer();
    assert_eq!(snapshot.policy().unwrap().peers, FACTORY);
    assert!(
        rendered(&snapshot, PEER).contains(
            r#""text":"<session-message from=\"22334455\">\n迁移写完了。\n</session-message>\n""#
        ),
        "{}",
        rendered(&snapshot, PEER)
    );
    let text = String::from_utf8(snapshot.to_bytes()).unwrap();
    assert!(
        text.contains(r#""peers":{"message_open":"<session-message from=\"{id}\">\n","message_close":"</session-message>\n"}"#),
        "{text}"
    );
    assert!(
        text.ends_with(r#""peers":{"burst":5,"window":600,"unread":50}}"#),
        "{text}"
    );
}

#[test]
fn the_numbers_in_the_snapshot_are_used() {
    let mut snapshot = engineer();
    snapshot.peers = Some(crate::PeerNumbers {
        burst: 2,
        window: 60,
        unread: 7,
    });
    assert_eq!(
        snapshot.policy().unwrap().peers,
        Peers {
            burst: 2,
            window: 60,
            unread: 7
        }
    );
}

#[test]
fn a_broken_tag_is_named() {
    let mut broken = engineer();
    if let Some(peers) = broken.core.peers.as_mut() {
        peers.message_open = "<session-message from=\"{who}\">\n".to_string();
    }
    let error = broken.policy().err().unwrap();
    assert!(
        matches!(
            error,
            BuildError::Texts {
                which: "session message texts",
                ..
            }
        ),
        "{error:?}"
    );
}

#[test]
fn older_snapshots_use_the_factory_numbers_and_render_it_like_the_persons_words() {
    let text = String::from_utf8(engineer().to_bytes()).unwrap();
    let older = without(&text, r#","peers":{"message_open""#, r#""}"#);
    let older = without(&older, r#","peers":{"burst""#, "}");
    let read = Snapshot::from_bytes(older.as_bytes()).unwrap();
    assert!(read.core.peers.is_none() && read.peers.is_none());
    assert_eq!(read.to_bytes(), older.as_bytes(), "读进来再写出去一字不差");
    assert_eq!(read.policy().unwrap().peers, FACTORY, "防刷屏照出厂的数");
    assert_eq!(
        rendered(&read, PEER),
        rendered(&read, ALICE),
        "没有标签：和人的话一字不差"
    );
}
