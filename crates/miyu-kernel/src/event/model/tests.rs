//! 模型调用的测试：图纸上的写法读写一字不差、认得出种类；出错的、第一处不同的写法；
//! 不认识的分类原样留着；指纹比出来的第一处不同怎么写。

use super::*;
use crate::event::Body;
use crate::test_support::{read_body, rejected};

const CALLED: &str = r#"{"seen":44,"endpoint":"deepseek","model":"deepseek-v4","request":"sha256:2b2966577ceda0727f654b534396fc3e5967b14bb226cdc374b2d6e0013c25f6","messages":1,"usage":{"uncached":1843,"cache_read":0,"cache_write":0,"output":26},"first_token_ms":812,"duration_ms":2760,"result":"ok"}"#;

fn called(body: &str) -> ModelCalled {
    match read_body("model.called", body) {
        Body::ModelCalled(called) => called,
        other => panic!("应该认得出 model.called：{other:?}"),
    }
}

#[test]
fn the_drawing_round_trips() {
    let called = called(CALLED);
    assert_eq!(called.seen.get(), 44);
    assert_eq!(called.messages, 1);
    assert_eq!(called.result, CallResult::Ok);
    assert_eq!(
        called.usage,
        Some(Usage {
            uncached: 1843,
            cache_read: 0,
            cache_write: 0,
            output: 26,
        })
    );
    assert_eq!(called.first_difference, None);
    assert_eq!(called.error, None);
}

#[test]
fn a_failed_call_before_it_was_sent_has_only_what_is_known() {
    let body = r#"{"seen":44,"messages":1,"result":"error","error":{"class":"rate_limited","message":"429 Too Many Requests"}}"#;
    let called = called(body);
    assert_eq!(
        (called.endpoint, called.model, called.request),
        (None, None, None)
    );
    assert_eq!((called.first_token_ms, called.duration_ms), (None, None));
    assert_eq!(
        called.error,
        Some(CallError {
            class: ErrorClass::RateLimited,
            message: "429 Too Many Requests".to_string(),
        })
    );
}

#[test]
fn each_error_class_reads_into_its_own_variant() {
    for (text, class) in [
        ("retryable", ErrorClass::Retryable),
        ("rate_limited", ErrorClass::RateLimited),
        ("context_too_long", ErrorClass::ContextTooLong),
        ("auth", ErrorClass::Auth),
        ("content_policy", ErrorClass::ContentPolicy),
        ("other", ErrorClass::Unclassified),
        ("bad_stream", ErrorClass::BadStream),
        ("empty_reply", ErrorClass::EmptyReply),
        ("overloaded", ErrorClass::Other("overloaded".to_string())),
    ] {
        let body = format!(
            r#"{{"seen":44,"messages":1,"result":"error","error":{{"class":"{text}","message":"…"}}}}"#
        );
        assert_eq!(called(&body).error.map(|error| error.class), Some(class));
    }
}

#[test]
fn first_differences_are_written_by_part() {
    for (difference, json) in [
        (Difference::Tools, r#"{"part":"tools"}"#),
        (Difference::System, r#"{"part":"system"}"#),
        (
            Difference::Message {
                index: 2,
                role: Role::User,
            },
            r#"{"part":"message","index":2,"role":"user"}"#,
        ),
        (
            Difference::Message {
                index: 0,
                role: Role::Tool,
            },
            r#"{"part":"message","index":0,"role":"tool"}"#,
        ),
    ] {
        let written = serde_json::to_string(&FirstDifference::from(difference)).unwrap();
        assert_eq!(written, json);
        let body = CALLED.replace(
            r#""messages":1,"#,
            &format!(r#""messages":1,"first_difference":{json},"#),
        );
        assert!(called(&body).first_difference.is_some());
    }
}

#[test]
fn broken_bodies_say_what_is_wrong() {
    let line = |body: &str| crate::test_support::event_line("model.called", body);
    rejected::<crate::event::Event>(
        &line(r#"{"messages":1,"result":"ok"}"#),
        "missing field `seen`",
    );
    rejected::<crate::event::Event>(
        &line(&CALLED.replace(r#""messages":1"#, r#""messages":-1"#)),
        "model.called 的 body 读不出来",
    );
}
