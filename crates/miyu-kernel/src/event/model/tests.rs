//! 模型调用的测试：图纸上的写法读写一字不差、认得出种类；出错的、第一处不同的写法；思考占了多少（施工 2-3 再补）；
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
            reasoning: None,
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
            status: None,
        })
    );
}

/// 出错带着 HTTP 状态码（施工 3-5 三补）：排在原话后面，读写一字不差；以前的日志没有这一格，照读，写出去还是
/// 没有，不写成 `null`。
#[test]
fn an_error_keeps_its_http_status_and_old_logs_without_it_still_read() {
    let body = r#"{"seen":44,"messages":1,"result":"error","error":{"class":"other","message":"HTTP 404: Model Not Exist","status":404}}"#;
    assert_eq!(
        called(body).error,
        Some(CallError {
            class: ErrorClass::Unclassified,
            message: "HTTP 404: Model Not Exist".to_string(),
            status: Some(404),
        })
    );
    let old = r#"{"seen":44,"messages":1,"result":"error","error":{"class":"rate_limited","message":"HTTP 429: Rate limit reached"}}"#;
    let error = called(old).error.expect("出错的有 error");
    assert_eq!(error.status, None);
    let written = serde_json::to_string(&error).unwrap();
    assert_eq!(
        written,
        r#"{"class":"rate_limited","message":"HTTP 429: Rate limit reached"}"#
    );
}

/// 回复每一块的起止（施工 2-3 补）：排在用时后面，读写一字不差；以前的日志没有这一格，照读，写出去还是没有，不写成
/// `null`。
/// 思考占了多少（施工 2-3 再补）：带着的读写一字不差；以前的日志没有这一格，读成没有，写出去照旧。
#[test]
fn reasoning_tokens_round_trip_and_old_logs_without_them_still_read() {
    let body = CALLED.replace(r#""output":26}"#, r#""output":26,"reasoning":20}"#);
    let thought = called(&body);
    assert_eq!(thought.usage.and_then(|usage| usage.reasoning), Some(20));
    assert_eq!(serde_json::to_string(&thought).unwrap(), body);
    let old = called(CALLED);
    assert_eq!(old.usage.and_then(|usage| usage.reasoning), None);
    assert_eq!(serde_json::to_string(&old).unwrap(), CALLED);
}

#[test]
fn block_spans_round_trip_and_old_logs_without_them_still_read() {
    let body = CALLED.replace(
        r#""duration_ms":2760,"#,
        r#""duration_ms":2760,"blocks":[{"start_ms":812,"end_ms":1490},{"start_ms":1502,"end_ms":2710}],"#,
    );
    let timed = called(&body);
    assert_eq!(
        timed.blocks,
        Some(vec![
            BlockSpan {
                start_ms: 812,
                end_ms: 1490,
            },
            BlockSpan {
                start_ms: 1502,
                end_ms: 2710,
            },
        ])
    );
    assert_eq!(serde_json::to_string(&timed).unwrap(), body);
    let old = called(CALLED);
    assert_eq!(old.blocks, None);
    assert_eq!(serde_json::to_string(&old).unwrap(), CALLED);
}

#[test]
fn a_summary_request_says_which_compaction_it_is_for() {
    // 施工 6-6 上：摘要请求多一格 `compaction`，排在最后；主请求没有，以前的日志读进来再写出去一字不差（上面那一条）。
    for (text, trigger) in [
        ("auto", CompactTrigger::Auto),
        ("manual", CompactTrigger::Manual),
        ("overflow", CompactTrigger::Overflow),
        ("scheduled", CompactTrigger::Other("scheduled".to_string())),
    ] {
        let body = format!(
            r#"{{"seen":44,"messages":1,"result":"error","error":{{"class":"bad_summary","message":"no summary in the reply"}},"compaction":"{text}"}}"#
        );
        let summary = called(&body);
        assert_eq!(summary.compaction, Some(trigger));
        assert_eq!(serde_json::to_string(&summary).unwrap(), body);
    }
    assert_eq!(called(CALLED).compaction, None);
}

/// 辅助请求多一格 `purpose`，排在最后（施工 3-8 四补）：回顾是 `recap`，起标题是 `title`（施工 3-8 五补），提前压好的摘要
/// 是 `compaction`（施工 6-11 上），不认识的原样
/// 留着，也算辅助请求；主请求、摘要请求没有这一格，不是辅助请求。
#[test]
fn an_aside_request_says_what_it_is_for() {
    for (text, purpose) in [
        ("recap", Purpose::Recap),
        ("title", Purpose::Title),
        ("compaction", Purpose::Compaction),
        ("vision", Purpose::Other("vision".to_string())),
    ] {
        let body = format!(r#"{{"seen":44,"messages":1,"result":"ok","purpose":"{text}"}}"#);
        let aside = called(&body);
        assert_eq!(aside.purpose, Some(purpose));
        assert!(aside.aside());
        assert_eq!(serde_json::to_string(&aside).unwrap(), body);
    }
    assert_eq!(called(CALLED).purpose, None);
    assert!(!called(CALLED).aside());
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
        ("bad_summary", ErrorClass::BadSummary),
        ("compaction_paused", ErrorClass::CompactionPaused),
        ("no_model", ErrorClass::NoModel),
        ("cooling", ErrorClass::Cooling),
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
        "body of model.called not readable",
    );
    // 块的起止两格都必有。
    rejected::<crate::event::Event>(
        &line(&CALLED.replace(r#""result""#, r#""blocks":[{"start_ms":812}],"result""#)),
        "body of model.called not readable",
    );
}

/// 金额排在用量后面（施工 8-15）：读写一字不差；以前的日志没有这一格，照读，写出去还是没有。
#[test]
fn a_cost_round_trips_after_the_usage_and_old_logs_without_it_still_read() {
    let body = CALLED.replace(
        r#""output":26},"#,
        r#""output":26},"cost":{"amount":0.00029205,"currency":"USD","price":{"input":0.15,"output":0.6,"cache_read":0.003},"multiplier":1,"source":"catalog:deepseek/deepseek-flash"},"#,
    );
    let priced = called(&body);
    let cost = priced.cost.as_deref().expect("有金额");
    assert_eq!(cost.amount.get(), 0.000_292_05);
    assert_eq!(cost.currency, "USD");
    assert_eq!(serde_json::to_string(&priced).unwrap(), body);
    let old = called(CALLED);
    assert_eq!(old.cost, None);
    assert_eq!(serde_json::to_string(&old).unwrap(), CALLED);
}
