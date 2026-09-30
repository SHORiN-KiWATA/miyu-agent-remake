//! `peer.idle` 的测试（施工 C-1）：图纸上的两行读写一字不差、认得出种类；每种 `reason` 读成自己那一种，不认识的原样
//! 留着；没有 `status` 的不写，写成 `null` 的当没有；坏的报错说清是哪一种。

use super::*;
use crate::event::{Body, Event};
use crate::test_support::{event_line, read_body, rejected};

/// 等的那个会话。
const PEER: &str = "0192f3a0-2222-7abc-8def-5566778899aa";

#[test]
fn the_notices_from_the_drawing_round_trip() {
    let idle = format!(
        r#"{{"session":"{PEER}","reason":"idle","status":"CI 修好了：macOS 上的临时目录换成了真实路径。"}}"#
    );
    match read_body("peer.idle", &idle) {
        Body::PeerIdle(notice) => assert_eq!(
            notice,
            PeerIdle {
                session: SessionId::parse(PEER).unwrap(),
                reason: IdleReason::Idle,
                status: Some("CI 修好了：macOS 上的临时目录换成了真实路径。".to_string()),
            }
        ),
        other => panic!("{other:?}"),
    }
    match read_body(
        "peer.idle",
        &format!(r#"{{"session":"{PEER}","reason":"expired"}}"#),
    ) {
        Body::PeerIdle(notice) => {
            assert_eq!(notice.reason, IdleReason::Expired);
            assert_eq!(notice.status, None, "没说话的不写");
        }
        other => panic!("{other:?}"),
    }
}

/// 不认识的原因是新版本才有的：原样留着，写出去还是原样。
#[test]
fn each_reason_reads_into_its_own_variant() {
    for (text, reason) in [
        ("idle", IdleReason::Idle),
        ("expired", IdleReason::Expired),
        ("gone", IdleReason::Gone),
        ("cancelled", IdleReason::Other("cancelled".to_string())),
    ] {
        let body = format!(r#"{{"session":"{PEER}","reason":"{text}"}}"#);
        match read_body("peer.idle", &body) {
            Body::PeerIdle(notice) => assert_eq!(notice.reason, reason),
            other => panic!("{other:?}"),
        }
    }
}

/// 可以没有的写成 `null`：读得进来，写出去不写。
#[test]
fn a_null_status_is_not_written() {
    let line = event_line(
        "peer.idle",
        &format!(r#"{{"session":"{PEER}","reason":"gone","status":null}}"#),
    );
    let tidy = event_line(
        "peer.idle",
        &format!(r#"{{"session":"{PEER}","reason":"gone"}}"#),
    );
    assert_eq!(Event::from_line(&line).unwrap().to_line(), tidy);
}

#[test]
fn broken_notices_say_which_kind() {
    for body in [
        r#"{"reason":"idle"}"#,
        r#"{"session":"s-1","reason":"idle"}"#,
        r#"{"session":"0192F3A0-2222-7abc-8def-5566778899aa","reason":"idle"}"#,
        r#"{"session":"0192f3a0-2222-7abc-8def-5566778899aa"}"#,
        r#"{"session":"0192f3a0-2222-7abc-8def-5566778899aa","reason":7}"#,
        r#"{"session":"0192f3a0-2222-7abc-8def-5566778899aa","reason":"idle","status":1}"#,
    ] {
        rejected::<Event>(
            &event_line("peer.idle", body),
            "body of peer.idle not readable",
        );
    }
}
