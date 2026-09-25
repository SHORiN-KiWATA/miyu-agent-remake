//! 回合事件的测试：图纸上的写法读写一字不差、认得出种类；每种结束原因认得出；
//! 不认识的原因原样留着；坏的报错说清是哪一种。

use super::*;
use crate::event::{Body, Event};
use crate::test_support::{event_line, read_body, rejected};

#[test]
fn turn_events_from_the_drawing_round_trip() {
    match read_body("turn.started", r#"{"trigger":41}"#) {
        Body::TurnStarted(started) => assert_eq!(started.trigger.get(), 41),
        other => panic!("{other:?}"),
    }
    match read_body("turn.reverted", r#"{"turns":[42,50]}"#) {
        Body::TurnReverted(reverted) => assert_eq!(reverted.turns.len(), 2),
        other => panic!("{other:?}"),
    }
}

#[test]
fn each_end_reason_reads_into_its_own_variant() {
    for (text, reason) in [
        ("completed", EndReason::Completed),
        ("interrupted", EndReason::Interrupted),
        ("error", EndReason::Error),
        ("step_limit", EndReason::StepLimit),
        ("aborted", EndReason::Aborted),
    ] {
        match read_body("turn.ended", &format!(r#"{{"reason":"{text}"}}"#)) {
            Body::TurnEnded(ended) => assert_eq!(ended.reason, reason),
            other => panic!("{other:?}"),
        }
    }
}

#[test]
fn an_unknown_end_reason_is_kept_as_it_is() {
    match read_body("turn.ended", r#"{"reason":"paused"}"#) {
        Body::TurnEnded(ended) => assert_eq!(ended.reason, EndReason::Other("paused".to_string())),
        other => panic!("{other:?}"),
    }
}

#[test]
fn broken_turn_bodies_say_which_kind() {
    let line = event_line("turn.started", r#"{"trigger":0}"#);
    rejected::<Event>(&line, "turn.started 的 body 读不出来");
    let line = event_line("turn.reverted", r#"{"turns":["42"]}"#);
    rejected::<Event>(&line, "turn.reverted 的 body 读不出来");
    let line = event_line("turn.ended", r#"{"reason":7}"#);
    rejected::<Event>(&line, "turn.ended 的 body 读不出来");
}
