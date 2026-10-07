//! 别的会话发来的话进快照（施工 C-2）：出厂的快照带着防刷屏的数和标签的两份，渲染出带短编号的标签；标签坏了照名字报；
//! 以前造的快照里没有数的照出厂的数，没有标签的那种话和人的话一字不差，读进来再写出去一字不差。
//!
//! 空了的通知（施工 C-6）：出厂的快照多两个数、通知的五份字，作废那一句照快照的小时数换；通知的字坏了照名字报；C-2 时造的
//! 快照没有这两个数、五份字，数照出厂的，通知不出，读进来再写出去一字不差。

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
    watch_hours: 12,
    status_chars: 200,
};

/// 等的会话 `22334455` 作废了的通知，开了第一轮以后到。
const EXPIRED: &str = r#"{"seq":4,"at":"2026-09-25T07:00:00.000Z","kind":"peer.idle","by":{"kind":"kernel"},"body":{"session":"0192f3a0-1111-7abc-8def-001122334455","reason":"expired"}}"#;

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

/// 照快照组装这段日志接一条作废了的通知，请求的字节。
fn with_notice(snapshot: &Snapshot) -> String {
    let mut history = history(ALICE);
    history.append(Event::from_line(EXPIRED).unwrap());
    let request = snapshot.policy().unwrap().assembler.assemble(&history);
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
    let text = crate::test_support::text_without_digest(&snapshot);
    assert!(
        text.contains(r#""peers":{"message_open":"<session-message from=\"{id}\">\n","message_close":"</session-message>\n","#),
        "{text}"
    );
    assert!(
        text.ends_with(
            r#""peers":{"burst":5,"window":600,"unread":50,"watch_hours":12,"status_chars":200}}"#
        ),
        "{text}"
    );
    assert!(
        text.contains(
            r#""idle_open":"<session-idle session=\"{id}\" reason=\"{reason}\">\n","idle_silent":"#
        ),
        "通知的字和标签平铺在一层：{text}"
    );
    assert!(
        with_notice(&snapshot).contains(r#"<session-idle session=\"22334455\" reason=\"expired\">\nNo notice came within 12 hours, so the request was dropped.\n</session-idle>\n"#),
        "{}",
        with_notice(&snapshot)
    );
}

#[test]
fn the_expiry_line_follows_the_hours_in_the_snapshot() {
    let mut snapshot = engineer();
    snapshot.peers = Some(crate::PeerNumbers {
        watch_hours: Some(3),
        ..crate::PEERS
    });
    assert!(
        with_notice(&snapshot).contains("within 3 hours"),
        "{}",
        with_notice(&snapshot)
    );
}

#[test]
fn a_broken_idle_text_is_named() {
    let mut broken = engineer();
    if let Some(idle) = broken
        .core
        .peers
        .as_mut()
        .and_then(|peers| peers.idle.as_mut())
    {
        idle.idle_expired = "No notice within {days} days.\n".to_string();
    }
    let error = broken.policy().err().unwrap();
    assert!(
        matches!(
            error,
            BuildError::Texts {
                which: "session idle texts",
                ..
            }
        ),
        "{error:?}"
    );
}

#[test]
fn snapshots_from_before_the_notices_keep_their_bytes_and_show_no_notice() {
    let text = String::from_utf8(engineer().to_bytes()).unwrap();
    let older = without(&text, r#","idle_open""#, r#"</session-idle>\n""#);
    let older = without(&older, r#","watch_hours""#, "200");
    let read = Snapshot::from_bytes(older.as_bytes()).unwrap();
    assert!(
        read.core
            .peers
            .as_ref()
            .is_some_and(|peers| peers.idle.is_none())
    );
    assert_eq!(read.to_bytes(), older.as_bytes(), "读进来再写出去一字不差");
    assert_eq!(read.policy().unwrap().peers, FACTORY, "数照出厂的");
    assert!(
        !with_notice(&read).contains("session-idle"),
        "订不了，通知不出：{}",
        with_notice(&read)
    );
}

#[test]
fn the_numbers_in_the_snapshot_are_used() {
    let mut snapshot = engineer();
    snapshot.peers = Some(crate::PeerNumbers {
        burst: 2,
        window: 60,
        unread: 7,
        watch_hours: Some(1),
        status_chars: Some(30),
    });
    assert_eq!(
        snapshot.policy().unwrap().peers,
        Peers {
            burst: 2,
            window: 60,
            unread: 7,
            watch_hours: 1,
            status_chars: 30,
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
