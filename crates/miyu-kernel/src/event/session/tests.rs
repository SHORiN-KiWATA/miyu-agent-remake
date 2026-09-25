//! 会话事件的测试：图纸上的写法读写一字不差、认得出种类；权限两格都要写；
//! 不认识的级别原样留着；坏的报错说清是哪一种。

use super::*;
use crate::event::{Body, Event};
use crate::test_support::{event_line, read_body, rejected};

const HASH: &str = "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

fn created(level: &str) -> String {
    format!(
        r#"{{"owner":"alice","venue":"local","policy":"{HASH}","permission":{{"level":"{level}","read_only":false}}}}"#
    )
}

#[test]
fn session_events_from_the_drawing_round_trip() {
    match read_body("session.created", &created("workspace")) {
        Body::SessionCreated(created) => assert_eq!(created.owner.as_str(), "alice"),
        other => panic!("{other:?}"),
    }
    read_body(
        "session.policy_changed",
        &format!(r#"{{"policy":"{HASH}"}}"#),
    );
    read_body(
        "session.policy_changed",
        r#"{"permission":{"level":"workspace","read_only":true}}"#,
    );
    read_body(
        "session.policy_changed",
        &format!(r#"{{"policy":"{HASH}","permission":{{"level":"full","read_only":false}}}}"#),
    );
    read_body("session.meta_changed", r#"{"title":"整理 src 目录"}"#);
    read_body("session.meta_changed", r#"{"pinned":true}"#);
}

#[test]
fn each_level_reads_into_its_own_variant() {
    for (text, level) in [("workspace", Level::Workspace), ("full", Level::Full)] {
        match read_body("session.created", &created(text)) {
            Body::SessionCreated(created) => assert_eq!(created.permission.level, level),
            other => panic!("{other:?}"),
        }
    }
}

#[test]
fn an_unknown_level_is_kept_as_it_is() {
    match read_body("session.created", &created("sandboxed")) {
        Body::SessionCreated(created) => {
            assert_eq!(
                created.permission.level,
                Level::Other("sandboxed".to_string())
            );
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn permission_needs_both_fields() {
    let line = event_line(
        "session.policy_changed",
        r#"{"permission":{"level":"workspace"}}"#,
    );
    rejected::<Event>(&line, "read_only");
}

#[test]
fn broken_session_bodies_say_which_kind() {
    let line = event_line(
        "session.created",
        &created("workspace").replace("alice", "Alice"),
    );
    rejected::<Event>(&line, "session.created 的 body 读不出来");
    let line = event_line("session.meta_changed", r#"{"pinned":"yes"}"#);
    rejected::<Event>(&line, "session.meta_changed 的 body 读不出来");
}
