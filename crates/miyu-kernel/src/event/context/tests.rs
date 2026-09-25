//! 上下文事件的测试：图纸上的两种读写一字不差、认得出种类；坏的报错说清是哪一种。

use crate::event::{Body, Event};
use crate::test_support::{event_line, read_body, rejected};

const INJECTED: &str = r#"{"kind":"env","text":"<env time=\"Fri 2026-09-25 16:00\" timezone=\"UTC+09:00\" cwd=\"~/src/miyu\"/>"}"#;
const COMPACTED: &str = r#"{"upto":52,"summary":"The user asked to look at the src directory. That turn was undone. Nothing is in progress."}"#;

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
        Body::ContextCompacted(compacted) => assert_eq!(compacted.upto.get(), 52),
        other => panic!("{other:?}"),
    }
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
        rejected::<Event>(&line, &format!("{kind} 的 body 读不出来"));
    }
}
