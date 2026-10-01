//! 会话事件的测试：图纸上的写法读写一字不差、认得出种类；权限两格都要写；
//! 不认识的级别原样留着；坏的报错说清是哪一种；子会话带着父会话和第几层（施工 7-1）；会话用哪个模型（施工 8-8）；
//! 换模型的两格（施工 8-10）；换思考强度的一格（施工 8-18）。

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
    match read_body("session.recapped", r#"{"text":"在打招呼。","upto":6}"#) {
        Body::SessionRecapped(recapped) => {
            assert_eq!(
                (recapped.text.as_str(), recapped.upto.get()),
                ("在打招呼。", 6)
            );
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_oneshot_session_says_so_and_others_do_not() {
    let oneshot = format!(
        r#"{{"owner":"alice","venue":"local","policy":"{HASH}","permission":{{"level":"workspace","read_only":false}},"oneshot":true}}"#
    );
    match read_body("session.created", &oneshot) {
        Body::SessionCreated(created) => assert!(created.oneshot),
        other => panic!("{other:?}"),
    }
    match read_body("session.created", &created("workspace")) {
        Body::SessionCreated(created) => assert!(!created.oneshot, "不写就不是"),
        other => panic!("{other:?}"),
    }
}

/// 子会话的 `session.created` 多两格（施工 7-1）：父会话、第几层，写在最后；主会话不写，原来的日志一个字节不变。
#[test]
fn a_child_session_names_its_parent_and_depth() {
    let parent = "01a0d75d-2180-7a3c-9e41-5b7d2c8f6a10";
    let child = format!(
        r#"{{"owner":"alice","venue":"local","policy":"{HASH}","permission":{{"level":"workspace","read_only":false}},"cwd":"~/src/miyu","parent":"{parent}","depth":1}}"#
    );
    match read_body("session.created", &child) {
        Body::SessionCreated(created) => {
            assert_eq!(
                created.parent.map(|p| p.to_string()),
                Some(parent.to_string())
            );
            assert_eq!(created.depth, Some(1));
        }
        other => panic!("{other:?}"),
    }
    match read_body("session.created", &created("workspace")) {
        Body::SessionCreated(created) => assert_eq!((created.parent, created.depth), (None, None)),
        other => panic!("{other:?}"),
    }
    // 写成 null 的当没有。
    let nulls = created("workspace").replace("}}", r#"},"parent":null,"depth":null}"#);
    let line = event_line("session.created", &nulls);
    let event = Event::from_line(&line).unwrap();
    assert_eq!(
        event.to_line(),
        event_line("session.created", &created("workspace"))
    );
    // 父会话不合写法、层数不是整数的，读不进来。
    for (bad, why) in [
        (r#""parent":"p-1","depth":1"#, "bad session id"),
        (
            &format!(r#""parent":"{parent}","depth":-1"#) as &str,
            "invalid value",
        ),
        (
            &format!(r#""parent":"{parent}","depth":"1""#) as &str,
            "invalid type",
        ),
    ] {
        let body = created("workspace").replace("}}", &format!("}},{bad}}}"));
        rejected::<Event>(&event_line("session.created", &body), why);
    }
}

/// 会话用哪个模型（施工 8-8）：写在最后，读写一字不差；以前的日志没有这一格，照没有读，写回去一个字节不变；`null` 当没有。
#[test]
fn a_session_records_its_model_and_old_logs_read_without_it() {
    let parent = "01a0d75d-2180-7a3c-9e41-5b7d2c8f6a10";
    for model in [
        "deepseek/deepseek-flash",
        "@free",
        "openrouter/deepseek/deepseek-v4",
    ] {
        let body = created("workspace").replace("}}", &format!(r#"}},"model":"{model}"}}"#));
        match read_body("session.created", &body) {
            Body::SessionCreated(created) => assert_eq!(created.model.as_deref(), Some(model)),
            other => panic!("{other:?}"),
        }
    }
    let child = format!(
        r#"{{"owner":"alice","venue":"local","policy":"{HASH}","permission":{{"level":"workspace","read_only":false}},"cwd":"~/src/miyu","parent":"{parent}","depth":1,"model":"@free"}}"#
    );
    read_body("session.created", &child);
    match read_body("session.created", &created("workspace")) {
        Body::SessionCreated(created) => assert_eq!(created.model, None, "以前的日志没有"),
        other => panic!("{other:?}"),
    }
    let nulled = created("workspace").replace("}}", r#"},"model":null}"#);
    let event = Event::from_line(&event_line("session.created", &nulled)).unwrap();
    assert_eq!(
        event.to_line(),
        event_line("session.created", &created("workspace"))
    );
    let wrong = created("workspace").replace("}}", r#"},"model":3}"#);
    rejected::<Event>(&event_line("session.created", &wrong), "invalid type");
}

/// 换模型（施工 8-10）：`model`、`replaced` 两格写在最后，读写一字不差；以前的日志没有这两格，照没有读，写回去一个字节不
/// 变；`null` 当没有；不是字的读不进来。
#[test]
fn a_policy_change_records_the_model_and_what_it_replaced() {
    for body in [
        r#"{"model":"@free"}"#,
        r#"{"model":"deepseek/deepseek-flash","replaced":"claude/opus"}"#,
        r#"{"permission":{"level":"workspace","read_only":true},"model":"a/m"}"#,
    ] {
        let line = event_line("session.policy_changed", body);
        let event = Event::from_line(&line).unwrap();
        assert_eq!(event.to_line(), line, "一字不差");
    }
    match read_body(
        "session.policy_changed",
        r#"{"model":"deepseek/deepseek-flash","replaced":"claude/opus"}"#,
    ) {
        Body::PolicyChanged(changed) => assert_eq!(
            (changed.model.as_deref(), changed.replaced.as_deref()),
            (Some("deepseek/deepseek-flash"), Some("claude/opus"))
        ),
        other => panic!("{other:?}"),
    }
    match read_body(
        "session.policy_changed",
        r#"{"permission":{"level":"full","read_only":false}}"#,
    ) {
        Body::PolicyChanged(changed) => {
            assert_eq!(
                (changed.model, changed.replaced),
                (None, None),
                "以前的日志"
            );
        }
        other => panic!("{other:?}"),
    }
    let nulled =
        r#"{"permission":{"level":"full","read_only":false},"model":null,"replaced":null}"#;
    let event = Event::from_line(&event_line("session.policy_changed", nulled)).unwrap();
    assert_eq!(
        event.to_line(),
        event_line(
            "session.policy_changed",
            r#"{"permission":{"level":"full","read_only":false}}"#
        )
    );
    for wrong in [r#"{"model":3}"#, r#"{"model":"a/m","replaced":["b/m"]}"#] {
        let line = event_line("session.policy_changed", wrong);
        rejected::<Event>(&line, "body of session.policy_changed not readable");
    }
}

/// 换思考强度（施工 8-18）：`effort` 写在最后，`level` 总是写（清掉的写 `null`），读写一字不差；和 `model` 能在同一条里；
/// 以前的日志没有这一格，照没有读；`effort` 写 `null` 当没有；`model` 不是字、`level` 不是字也不是 `null` 的读不进来。
#[test]
fn a_policy_change_records_an_effort_cell() {
    for body in [
        r#"{"effort":{"model":"deepseek/deepseek-v4","level":"high"}}"#,
        r#"{"effort":{"model":"deepseek/deepseek-v4","level":null}}"#,
        r#"{"model":"@free","effort":{"model":"a/m","level":"off"}}"#,
    ] {
        let line = event_line("session.policy_changed", body);
        let event = Event::from_line(&line).unwrap();
        assert_eq!(event.to_line(), line, "一字不差");
    }
    match read_body(
        "session.policy_changed",
        r#"{"effort":{"model":"deepseek/deepseek-v4","level":null}}"#,
    ) {
        Body::PolicyChanged(changed) => assert_eq!(
            changed.effort,
            Some(Effort {
                model: "deepseek/deepseek-v4".to_string(),
                level: None,
            }),
            "清掉的是 null"
        ),
        other => panic!("{other:?}"),
    }
    match read_body("session.policy_changed", r#"{"model":"a/m"}"#) {
        Body::PolicyChanged(changed) => assert_eq!(changed.effort, None, "以前的日志"),
        other => panic!("{other:?}"),
    }
    let nulled = event_line("session.policy_changed", r#"{"model":"a/m","effort":null}"#);
    assert_eq!(
        Event::from_line(&nulled).unwrap().to_line(),
        event_line("session.policy_changed", r#"{"model":"a/m"}"#)
    );
    for wrong in [
        r#"{"effort":{"model":3,"level":"high"}}"#,
        r#"{"effort":{"model":"a/m","level":3}}"#,
        r#"{"effort":"high"}"#,
    ] {
        let line = event_line("session.policy_changed", wrong);
        rejected::<Event>(&line, "body of session.policy_changed not readable");
    }
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
    rejected::<Event>(&line, "body of session.created not readable");
    let line = event_line("session.meta_changed", r#"{"pinned":"yes"}"#);
    rejected::<Event>(&line, "body of session.meta_changed not readable");
    // 回顾两格都要写（施工 3-8 四补）。
    for body in [r#"{"upto":6}"#, r#"{"text":"x"}"#] {
        let line = event_line("session.recapped", body);
        rejected::<Event>(&line, "body of session.recapped not readable");
    }
}
