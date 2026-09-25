//! 工具事件的测试：图纸上的 `tool.result` 读写一字不差、认得出种类；每种状态认得出；
//! 不认识的状态原样留着；没真执行过的没有用时；坏的报错说清是哪一种。

use super::*;
use crate::event::{Body, Event};
use crate::test_support::{event_line, read_body, rejected};

const RESULT: &str = r#"{"call_id":"call_44_1","status":"ok","blocks":[{"type":"text","text":"lib.rs\nmain.rs"}],"duration_ms":12}"#;

/// 一条没有内容、没有用时的结果，只有状态不同。
fn result_with(status: &str) -> String {
    format!(r#"{{"call_id":"call_44_1","status":"{status}","blocks":[]}}"#)
}

#[test]
fn tool_result_from_the_drawing_round_trips() {
    match read_body("tool.result", RESULT) {
        Body::ToolResult(result) => {
            assert_eq!(result.call_id.to_string(), "call_44_1");
            assert_eq!(result.status, ToolStatus::Ok);
            assert_eq!(result.duration_ms, Some(12));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn each_status_reads_into_its_own_variant() {
    for (text, status) in [
        ("ok", ToolStatus::Ok),
        ("error", ToolStatus::Error),
        ("cancelled", ToolStatus::Cancelled),
        ("denied", ToolStatus::Denied),
        ("skipped", ToolStatus::Skipped),
    ] {
        match read_body("tool.result", &result_with(text)) {
            Body::ToolResult(result) => assert_eq!(result.status, status),
            other => panic!("{other:?}"),
        }
    }
}

#[test]
fn an_unknown_status_is_kept_as_it_is() {
    match read_body("tool.result", &result_with("timed_out")) {
        Body::ToolResult(result) => {
            assert_eq!(result.status, ToolStatus::Other("timed_out".to_string()));
        }
        other => panic!("{other:?}"),
    }
}

/// 没真执行过的没有用时这一格：读进来是没有，写出去也不写（read_body 查了一字不差）。
#[test]
fn a_result_that_never_ran_has_no_duration() {
    match read_body("tool.result", &result_with("skipped")) {
        Body::ToolResult(result) => assert_eq!(result.duration_ms, None),
        other => panic!("{other:?}"),
    }
}

#[test]
fn broken_tool_results_say_which_kind() {
    // 调用编号不合写法；少了状态；用时是负数、是小数。
    for body in [
        result_with("ok").replace("call_44_1", "c1"),
        r#"{"call_id":"call_44_1","blocks":[]}"#.to_string(),
        RESULT.replace(r#""duration_ms":12"#, r#""duration_ms":-1"#),
        RESULT.replace(r#""duration_ms":12"#, r#""duration_ms":1.5"#),
    ] {
        let line = event_line("tool.result", &body);
        rejected::<Event>(&line, "tool.result 的 body 读不出来");
    }
}
