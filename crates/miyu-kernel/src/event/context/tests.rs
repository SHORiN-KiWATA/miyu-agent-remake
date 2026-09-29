//! 上下文事件的测试：图纸上的两种读写一字不差、认得出种类；压缩的几种原因、代码写的几段和重读的文件；坏的报错说清是
//! 哪一种。

use crate::event::{Body, CompactTrigger, Event};
use crate::test_support::{event_line, read_body, rejected};

const INJECTED: &str = r#"{"kind":"env","text":"<env time=\"Fri 2026-09-25 16:00–17:00\" timezone=\"UTC+09:00\" cwd=\"~/src/miyu\"/>"}"#;
const COMPACTED: &str = r#"{"upto":53,"summary":"The user asked to look at the src directory. That turn was undone. Nothing is in progress."}"#;

#[test]
fn context_events_from_the_drawing_round_trip() {
    match read_body("context.injected", INJECTED) {
        Body::ContextInjected(injected) => {
            assert_eq!(injected.kind.as_str(), "env");
            assert!(injected.text.starts_with("<env "), "{}", injected.text);
        }
        other => panic!("{other:?}"),
    }
    match read_body("context.compacted", COMPACTED) {
        Body::ContextCompacted(compacted) => {
            assert_eq!(compacted.upto.get(), 53);
            assert_eq!(compacted.trigger, None);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn each_compaction_trigger_is_written_and_read_back() {
    for (text, trigger) in [
        ("auto", CompactTrigger::Auto),
        ("manual", CompactTrigger::Manual),
        ("overflow", CompactTrigger::Overflow),
        ("scheduled", CompactTrigger::Other("scheduled".to_string())),
    ] {
        let body = format!(r#"{{"upto":53,"summary":"S","trigger":"{text}"}}"#);
        let Body::ContextCompacted(compacted) = read_body("context.compacted", &body) else {
            panic!("{body}");
        };
        assert_eq!(compacted.trigger, Some(trigger));
        assert_eq!(serde_json::to_string(&compacted).unwrap(), body);
    }
    // 没有的不写：以前的日志读进来再写出去一字不差。
    let Body::ContextCompacted(old) = read_body("context.compacted", COMPACTED) else {
        panic!("{COMPACTED}");
    };
    assert_eq!(serde_json::to_string(&old).unwrap(), COMPACTED);
}

#[test]
fn notes_and_restored_files_are_written_and_read_back() {
    // 施工 6-5：代码写的几段、压完重读的文件。没有的不写（上面那一条守着）。
    let body = r#"{"upto":53,"summary":"S","trigger":"auto","notes":"Entries 1-53 were compacted.\n","restored":[{"path":"src/lib.rs","blob":"sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","tokens":1200}]}"#;
    let Body::ContextCompacted(compacted) = read_body("context.compacted", body) else {
        panic!("{body}");
    };
    assert_eq!(compacted.notes, "Entries 1-53 were compacted.\n");
    assert_eq!(compacted.restored.len(), 1);
    assert_eq!(compacted.restored[0].path, "src/lib.rs");
    assert_eq!(compacted.restored[0].tokens, 1200);
    assert_eq!(serde_json::to_string(&compacted).unwrap(), body);
}

#[test]
fn broken_context_bodies_say_which_kind() {
    for (kind, body) in [
        // 类别不合模块名的规矩；少了原文。
        ("context.injected", r#"{"kind":"Env","text":"<env/>"}"#),
        ("context.injected", r#"{"kind":"env"}"#),
        // 序号从 1 开始；少了摘要。
        ("context.compacted", r#"{"upto":0,"summary":""}"#),
        ("context.compacted", r#"{"upto":52}"#),
    ] {
        let line = event_line(kind, body);
        rejected::<Event>(&line, &format!("body of {kind} not readable"));
    }
}
