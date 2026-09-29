//! 两种回报的测试（施工 7-1）：图纸上的写法读写一字不差、认得出种类；每种 `reason` 读成自己那一种，不认识的原样
//! 留着；不写是假的几格是假时不写、没有的格不写；退出码带符号；坏的报错说清是哪一种。

use super::*;
use crate::event::{Body, Event};
use crate::test_support::{event_line, read_body, rejected};

const CHILD: &str = "01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91";
const OUTPUT: &str = "sha256:3245795a51c1f9e64adb119c6b258a9079a4fdc7f1a15677f1d15d09910bef9c";

#[test]
fn job_reported_from_the_drawing_round_trips() {
    let exited = format!(
        r#"{{"job":"j1","reason":"exited","exit_code":0,"duration_ms":81234,"output":"{OUTPUT}","chars":48213}}"#
    );
    match read_body("job.reported", &exited) {
        Body::JobReported(reported) => assert_eq!(
            reported,
            JobReported {
                job: JobId::parse("j1").unwrap(),
                reason: JobReason::Exited,
                exit_code: Some(0),
                signal: None,
                by_model: false,
                duration_ms: Some(81234),
                output: Some(ContentHash::parse(OUTPUT).unwrap()),
                chars: Some(48213),
            }
        ),
        other => panic!("{other:?}"),
    }
    // 被信号杀掉的：没有退出码，写信号。
    read_body(
        "job.reported",
        &format!(
            r#"{{"job":"j1","reason":"exited","signal":9,"duration_ms":5,"output":"{OUTPUT}","chars":0}}"#
        ),
    );
    // 她自己用 jobs 停的。
    match read_body(
        "job.reported",
        r#"{"job":"j3","reason":"stopped","signal":15,"by_model":true,"duration_ms":1200}"#,
    ) {
        Body::JobReported(reported) => assert!(reported.by_model),
        other => panic!("{other:?}"),
    }
    // 载入时补的：只知道是哪一个、为什么。
    match read_body("job.reported", r#"{"job":"j4","reason":"aborted"}"#) {
        Body::JobReported(reported) => {
            assert!(!reported.by_model, "不写是假");
            assert_eq!(
                (reported.exit_code, reported.signal, reported.duration_ms),
                (None, None, None)
            );
            assert_eq!((reported.output, reported.chars), (None, None));
        }
        other => panic!("{other:?}"),
    }
}

/// Windows 上的 NTSTATUS 退出码是负的，照 `ExitStatus::code()` 原样记（`kernel/events-bodies.md`）。
#[test]
fn an_exit_code_can_be_negative() {
    match read_body(
        "job.reported",
        r#"{"job":"j1","reason":"exited","exit_code":-1073741819}"#,
    ) {
        Body::JobReported(reported) => assert_eq!(reported.exit_code, Some(-1_073_741_819)),
        other => panic!("{other:?}"),
    }
}

#[test]
fn each_job_reason_reads_into_its_own_variant() {
    for (text, reason) in [
        ("exited", JobReason::Exited),
        ("stopped", JobReason::Stopped),
        ("undone", JobReason::Undone),
        ("restarted", JobReason::Restarted),
        ("aborted", JobReason::Aborted),
        ("evicted", JobReason::Other("evicted".to_string())),
    ] {
        match read_body(
            "job.reported",
            &format!(r#"{{"job":"j1","reason":"{text}"}}"#),
        ) {
            Body::JobReported(reported) => assert_eq!(reported.reason, reason),
            other => panic!("{other:?}"),
        }
    }
}

#[test]
fn child_reported_from_the_drawing_round_trips() {
    let done =
        format!(r#"{{"job":"j2","session":"{CHILD}","reason":"done","text":"CI 红是因为……"}}"#);
    match read_body("child.reported", &done) {
        Body::ChildReported(reported) => assert_eq!(
            reported,
            ChildReported {
                job: JobId::parse("j2").unwrap(),
                session: SessionId::parse(CHILD).unwrap(),
                reason: ChildReason::Done,
                text: "CI 红是因为……".to_string(),
                truncated: false,
                person: false,
                by_model: false,
            }
        ),
        other => panic!("{other:?}"),
    }
    // 截过的、人插过话的，两格写 true；一个字都没说就结束的，正文是空的。
    match read_body(
        "child.reported",
        &format!(
            r#"{{"job":"j2","session":"{CHILD}","reason":"done","text":"头……尾","truncated":true,"person":true}}"#
        ),
    ) {
        Body::ChildReported(reported) => assert!(reported.truncated && reported.person),
        other => panic!("{other:?}"),
    }
    read_body(
        "child.reported",
        &format!(r#"{{"job":"j2","session":"{CHILD}","reason":"stopped","text":""}}"#),
    );
    // 她自己用 `jobs` 停的（施工 7-4）：写 `by_model`，读写一字不差。
    let stopped = format!(
        r#"{{"job":"j2","session":"{CHILD}","reason":"stopped","text":"","by_model":true}}"#
    );
    let line = event_line("child.reported", &stopped);
    let event = Event::from_line(&line).unwrap();
    match &event.body {
        Body::ChildReported(reported) => assert!(reported.by_model),
        other => panic!("{other:?}"),
    }
    assert_eq!(event.to_line(), line);
}

#[test]
fn each_child_reason_reads_into_its_own_variant() {
    for (text, reason) in [
        ("done", ChildReason::Done),
        ("stopped", ChildReason::Stopped),
        ("undone", ChildReason::Undone),
        ("aborted", ChildReason::Aborted),
        ("handed_off", ChildReason::Other("handed_off".to_string())),
    ] {
        let body = format!(r#"{{"job":"j2","session":"{CHILD}","reason":"{text}","text":""}}"#);
        match read_body("child.reported", &body) {
            Body::ChildReported(reported) => assert_eq!(reported.reason, reason),
            other => panic!("{other:?}"),
        }
    }
}

/// 不写是假的写成 `false`、可以没有的写成 `null`：读得进来，写出去都不写。
#[test]
fn false_and_absent_fields_are_not_written() {
    let line = event_line(
        "child.reported",
        &format!(
            r#"{{"job":"j2","session":"{CHILD}","reason":"done","text":"好了","truncated":false,"person":false,"by_model":false}}"#
        ),
    );
    let tidy = event_line(
        "child.reported",
        &format!(r#"{{"job":"j2","session":"{CHILD}","reason":"done","text":"好了"}}"#),
    );
    assert_eq!(Event::from_line(&line).unwrap().to_line(), tidy);
    let line = event_line(
        "job.reported",
        r#"{"job":"j1","reason":"aborted","exit_code":null,"signal":null,"by_model":false,"duration_ms":null,"output":null,"chars":null}"#,
    );
    let tidy = event_line("job.reported", r#"{"job":"j1","reason":"aborted"}"#);
    assert_eq!(Event::from_line(&line).unwrap().to_line(), tidy);
}

#[test]
fn broken_reports_say_which_kind() {
    for body in [
        r#"{"reason":"exited"}"#,
        r#"{"job":"j0","reason":"exited"}"#,
        r#"{"job":"1","reason":"exited"}"#,
        r#"{"job":"j1"}"#,
        r#"{"job":"j1","reason":"exited","exit_code":1.5}"#,
        r#"{"job":"j1","reason":"exited","exit_code":4294967295}"#,
        r#"{"job":"j1","reason":"exited","signal":-9}"#,
        r#"{"job":"j1","reason":"exited","output":"md5:00"}"#,
        r#"{"job":"j1","reason":"exited","chars":-1}"#,
        r#"{"job":"j1","reason":"stopped","by_model":"yes"}"#,
    ] {
        rejected::<Event>(
            &event_line("job.reported", body),
            "body of job.reported not readable",
        );
    }
    for body in [
        r#"{"session":"01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91","reason":"done","text":""}"#,
        r#"{"job":"j2","reason":"done","text":""}"#,
        r#"{"job":"j2","session":"s-1","reason":"done","text":""}"#,
        r#"{"job":"j2","session":"01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91","text":""}"#,
        r#"{"job":"j2","session":"01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91","reason":"done"}"#,
        r#"{"job":"j2","session":"01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91","reason":"done","text":7}"#,
        r#"{"job":"j2","session":"01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91","reason":"done","text":"","person":1}"#,
    ] {
        rejected::<Event>(
            &event_line("child.reported", body),
            "body of child.reported not readable",
        );
    }
}
