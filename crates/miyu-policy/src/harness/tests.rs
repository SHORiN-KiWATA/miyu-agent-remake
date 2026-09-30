//! 别的 harness 发来的话的标签进快照（施工 7-10）：出厂的快照带着两份，渲染出带名字的标签；标签坏了照名字报；以前造的
//! 快照里没有，读成没有，那种话和人的话一字不差，读进来再写出去一字不差。

use miyu_kernel::event::Event;
use miyu_kernel::history::History;

use crate::snapshot::{BuildError, Snapshot};
use crate::test_support::engineer;

/// Claude Code 开了第一轮的日志。
fn history(by: &str) -> History {
    let lines = [
        r#"{"seq":1,"at":"2026-09-25T07:00:00.000Z","kind":"session.created","by":{"kind":"kernel"},"body":{"owner":"alice","venue":"local","policy":"sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","permission":{"level":"workspace","read_only":false}}}"#.to_string(),
        format!(
            r#"{{"seq":2,"at":"2026-09-25T07:00:00.000Z","kind":"message.user","by":{by},"cause":"c1","body":{{"blocks":[{{"type":"text","text":"CI 修好了。"}}]}}}}"#
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

const CLAUDE: &str = r#"{"kind":"harness","name":"claude-code"}"#;
const ALICE: &str = r#"{"kind":"person","account":"alice"}"#;

#[test]
fn the_tags_go_in_and_are_rendered() {
    let snapshot = engineer();
    assert!(
        rendered(&snapshot, CLAUDE).contains(
            r#""text":"<agent-message from=\"claude-code\">\nCI 修好了。\n</agent-message>\n""#
        ),
        "{}",
        rendered(&snapshot, CLAUDE)
    );
    let text = String::from_utf8(snapshot.to_bytes()).unwrap();
    assert!(
        text.contains(r#""harness":{"message_open":"<agent-message from=\"{name}\">\n","message_close":"</agent-message>\n"}"#),
        "{text}"
    );
}

#[test]
fn a_broken_tag_is_named() {
    let mut broken = engineer();
    if let Some(harness) = broken.core.harness.as_mut() {
        harness.message_open = "<agent-message from=\"{who}\">\n".to_string();
    }
    let error = broken.policy().err().unwrap();
    assert!(
        matches!(
            error,
            BuildError::Texts {
                which: "harness message texts",
                ..
            }
        ),
        "{error:?}"
    );
}

#[test]
fn older_snapshots_lack_the_tags_and_render_it_like_the_persons_words() {
    let text = String::from_utf8(engineer().to_bytes()).unwrap();
    let start = text.find(r#","harness":{"#).unwrap();
    let end = start + text[start..].find(r#""}"#).unwrap() + 2;
    let older = text[..start].to_string() + &text[end..];
    let read = Snapshot::from_bytes(older.as_bytes()).unwrap();
    assert!(read.core.harness.is_none());
    assert_eq!(read.to_bytes(), older.as_bytes(), "读进来再写出去一字不差");
    assert_eq!(
        rendered(&read, CLAUDE),
        rendered(&read, ALICE),
        "没有标签：和人的话一字不差"
    );
}
