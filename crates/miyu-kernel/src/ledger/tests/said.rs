//! 账本里最近收下的别的会话的话（施工 C-2，`kernel/history.md`「最近收下的别的会话的话」，`cross-session.md` 第五条第 5
//! 款）：别的会话是既不是父会话、也不是派的子代理的会话；它的话记下时刻和哈希，照时刻数；还没听到的，主对话的请求、回复
//! 看到了才不算，回顾的请求、压缩的摘要请求、暂停着没发出去的那一条不算听到。
//!
//! 底子是一个子会话（父会话 `P`）：第 3 轮派了子代理（会话 `A`），回合结束，闲着。

use super::*;
use crate::id::{ContentHash, SessionId};

/// 父会话。
const P: &str = "01a0d75d-2180-7a3c-9e41-5b7d2c8f6a10";
/// 这个会话派的子代理的会话。
const A: &str = "01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91";
/// 别的会话。
const C: &str = "0192f3a0-1111-7abc-8def-001122334455";

fn id(text: &str) -> SessionId {
    SessionId::parse(text).unwrap()
}

/// 拼一条事件：时刻是 07:00 过 `second` 秒，`by` 是会话 `from`（没有的是内核）。
fn line(
    seq: u64,
    second: u64,
    turn: Option<u64>,
    kind: &str,
    from: Option<&str>,
    body: &str,
) -> Event {
    let turn = turn.map(|t| format!(r#""turn":{t},"#)).unwrap_or_default();
    let by = from.map_or(r#"{"kind":"kernel"}"#.to_string(), |from| {
        format!(r#"{{"kind":"session","id":"{from}"}}"#)
    });
    Event::from_line(&format!(
        r#"{{"seq":{seq},"at":"2026-09-25T07:00:{second:02}.000Z","kind":"{kind}",{turn}"by":{by},"body":{body}}}"#
    ))
    .unwrap()
}

/// 一句话的 `body`。
fn words(text: &str) -> String {
    format!(r#"{{"blocks":[{{"type":"text","text":"{text}"}}]}}"#)
}

/// 底子：前 7 条。
fn opening() -> Vec<Event> {
    let created = format!(
        r#"{{"owner":"alice","venue":"local","policy":"sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","permission":{{"level":"workspace","read_only":false}},"parent":"{P}","depth":1}}"#
    );
    let started = format!(
        r#"{{"call_id":"call_4_1","status":"ok","blocks":[],"effects":[{{"kind":"job.started","job":"j1","what":"agent","title":"t","session":"{A}"}}]}}"#
    );
    vec![
        line(1, 1, None, "session.created", None, &created),
        line(2, 2, None, "message.user", Some(P), &words("查一下")),
        line(3, 3, Some(3), "turn.started", None, r#"{"trigger":2}"#),
        line(
            4,
            4,
            Some(3),
            "message.assistant",
            None,
            &reply(4, 3, 1, false),
        ),
        line(5, 5, Some(3), "tool.result", None, &started),
        line(
            6,
            6,
            Some(3),
            "message.assistant",
            None,
            &reply(6, 5, 0, false),
        ),
        line(
            7,
            7,
            Some(3),
            "turn.ended",
            None,
            r#"{"reason":"completed"}"#,
        ),
    ]
}

/// 底子追加完的账本，再追加 `more`。
fn ledger(more: &[Event]) -> Ledger {
    let mut ledger = Ledger::default();
    for event in opening().iter().chain(more) {
        ledger.append(event).unwrap();
    }
    ledger
}

#[test]
fn a_peer_is_neither_the_parent_nor_a_subagent() {
    let ledger = ledger(&[]);
    assert!(!ledger.is_peer(&id(P)), "父会话");
    assert!(!ledger.is_peer(&id(A)), "派的子代理");
    assert!(ledger.is_peer(&id(C)));
    assert!(
        Ledger::default().is_peer(&id(P)),
        "还不知道父会话的，都是别的会话"
    );
}

#[test]
fn only_peer_messages_are_recorded_by_their_moment() {
    let ledger = ledger(&[
        line(8, 8, None, "message.user", Some(C), &words("一")),
        line(9, 9, None, "message.user", Some(C), &words("二")),
        line(10, 10, None, "message.user", Some(P), &words("父会话的")),
        line(11, 11, None, "message.user", Some(A), &words("子代理的")),
    ]);
    let moment = |second: u64| {
        crate::time::Timestamp::parse(&format!("2026-09-25T07:00:{second:02}.000Z"))
            .unwrap()
            .unix_millis()
    };
    let said: Vec<&ContentHash> = ledger.peer_said(&id(C), moment(8)).collect();
    let blocks = |text: &str| {
        vec![Block::Text(crate::block::Text {
            text: text.to_string(),
        })]
    };
    assert_eq!(
        said,
        [&digest(&blocks("一")), &digest(&blocks("二"))],
        "含正好那一刻"
    );
    assert_eq!(ledger.peer_said(&id(C), moment(8) + 1).count(), 1);
    assert_eq!(ledger.peer_said(&id(P), 0).count(), 0, "父会话的不记");
    assert_eq!(ledger.peer_said(&id(A), 0).count(), 0, "子代理的不记");
    assert_eq!(ledger.unheard_from_peers(), 2, "只算别的会话的");
}

#[test]
fn peer_messages_are_unheard_until_a_main_request_sees_them() {
    let recap = r#"{"seen":9,"messages":1,"result":"ok","purpose":"recap"}"#;
    let mut ledger = ledger(&[
        line(8, 8, None, "message.user", Some(C), &words("一")),
        line(9, 9, None, "message.user", Some(C), &words("二")),
        line(10, 10, None, "model.called", None, recap),
    ]);
    assert_eq!(ledger.unheard_from_peers(), 2, "回顾的请求不算听到");
    for event in [
        line(11, 11, Some(11), "turn.started", None, r#"{"trigger":9}"#),
        line(
            12,
            12,
            Some(11),
            "model.called",
            None,
            r#"{"seen":9,"messages":1,"result":"ok","compaction":"auto"}"#,
        ),
        line(
            13,
            13,
            Some(11),
            "model.called",
            None,
            r#"{"seen":9,"messages":1,"result":"error","error":{"class":"compaction_paused","message":"paused"}}"#,
        ),
    ] {
        ledger.append(&event).unwrap();
    }
    assert_eq!(
        ledger.unheard_from_peers(),
        2,
        "压缩的摘要请求、暂停着没发出去的不算听到"
    );
    let called = r#"{"seen":8,"messages":1,"result":"ok"}"#;
    ledger
        .append(&line(14, 14, Some(11), "model.called", None, called))
        .unwrap();
    assert_eq!(ledger.unheard_from_peers(), 1, "请求看到哪里算到哪里");
    ledger
        .append(&line(
            15,
            15,
            Some(11),
            "message.assistant",
            None,
            &reply(15, 14, 0, false),
        ))
        .unwrap();
    assert_eq!(ledger.unheard_from_peers(), 0, "回复看到了");
}
