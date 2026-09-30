//! 跨会话的几条规矩（施工 C-1，`kernel/history.md`「账本查的规矩」，`cross-session.md`「效果 peer.watch」「事件
//! peer.idle」）：订的是自己的拒；`peer.idle` 只认在等的、`by` 对得上；撤掉订它的那一轮就不算在等；再订从新的时刻算。
//!
//! 底子是这个会话（`OWN`）的一段：第 3 轮一次调用订了会话 B（第 5 条，07:00:05），回合结束。

use super::*;
use crate::id::SessionId;
use crate::time::Timestamp;

/// 这个会话。
const OWN: &str = "01a0d75d-2180-7a3c-9e41-5b7d2c8f6a10";
/// 被等的会话。
const B: &str = "0192f3a0-2222-7abc-8def-5566778899aa";
/// 另一个会话。
const C: &str = "01a0cf12-7e40-7d1b-8c3a-51d49f03b21c";
/// 内核做的 `by`。
const KERNEL: &str = r#"{"kind":"kernel"}"#;
/// 人做的 `by`。
const ALICE: &str = r#"{"kind":"person","account":"alice"}"#;

/// 07:00 过 `second` 秒。
fn at(second: u64) -> Timestamp {
    Timestamp::parse(&format!("2026-09-25T07:00:{second:02}.000Z")).unwrap()
}

fn id(text: &str) -> SessionId {
    SessionId::parse(text).unwrap()
}

/// 拼一条事件：时刻是 07:00 过 `second` 秒，`by` 照写。
fn line(seq: u64, second: u64, turn: Option<u64>, kind: &str, by: &str, body: &str) -> Event {
    let turn = turn.map(|t| format!(r#""turn":{t},"#)).unwrap_or_default();
    Event::from_line(&format!(
        r#"{{"seq":{seq},"at":"{}","kind":"{kind}",{turn}"by":{by},"body":{body}}}"#,
        at(second)
    ))
    .unwrap()
}

/// 会话 `id` 做的 `by`。
fn session(id: &str) -> String {
    format!(r#"{{"kind":"session","id":"{id}"}}"#)
}

/// 效果 `peer.watch`。
fn watch(session: &str) -> String {
    format!(r#"{{"kind":"peer.watch","session":"{session}"}}"#)
}

/// 调用 `call` 的结果，带着几样效果。
fn watched(call: &str, effects: &[String]) -> String {
    format!(
        r#"{{"call_id":"{call}","status":"ok","blocks":[],"effects":[{}]}}"#,
        effects.join(",")
    )
}

/// 会话 `session` 的 `peer.idle`，原因是 `reason`，第 `seq` 条，07:00 过 `seq` 秒，不带回合编号。
fn notice(seq: u64, by: &str, session: &str, reason: &str) -> Event {
    let body = format!(r#"{{"session":"{session}","reason":"{reason}"}}"#);
    line(seq, seq, None, "peer.idle", by, &body)
}

/// 底子：前 7 条，第 3 轮订了 B。
fn opening() -> Vec<Event> {
    vec![
        event(1, None, "session.created", CREATED),
        event(2, None, "message.user", SAID),
        event(3, Some(3), "turn.started", r#"{"trigger":2}"#),
        event(4, Some(3), "message.assistant", &reply(4, 3, 1, false)),
        line(
            5,
            5,
            Some(3),
            "tool.result",
            KERNEL,
            &watched("call_4_1", &[watch(B)]),
        ),
        event(6, Some(3), "message.assistant", &reply(6, 5, 0, false)),
        event(7, Some(3), "turn.ended", r#"{"reason":"completed"}"#),
    ]
}

/// 这个会话的账本，追加了底子的前 `n` 条。
fn watching_after(n: usize) -> Ledger {
    let mut ledger = Ledger::for_session(id(OWN));
    for event in &opening()[..n] {
        ledger.append(event).unwrap();
    }
    ledger
}

/// 账本说在等的：会话，从哪一刻算起。
fn waited(ledger: &Ledger) -> Vec<(SessionId, Timestamp)> {
    ledger
        .watching()
        .map(|(session, since)| (session.clone(), since))
        .collect()
}

/// 从第 `first` 条起开一轮，里面一次调用又订了 B，回合结束：六条，订的那一条在 `first + 3`，时刻是 07:00 过那么多秒。
fn watch_again(ledger: &mut Ledger, first: u64) {
    let turn = first + 1;
    let call = format!("call_{}_1", first + 2);
    for event in [
        event(first, None, "message.user", SAID),
        event(
            turn,
            Some(turn),
            "turn.started",
            &format!(r#"{{"trigger":{first}}}"#),
        ),
        event(
            first + 2,
            Some(turn),
            "message.assistant",
            &reply(first + 2, turn, 1, false),
        ),
        line(
            first + 3,
            first + 3,
            Some(turn),
            "tool.result",
            KERNEL,
            &watched(&call, &[watch(B)]),
        ),
        event(
            first + 4,
            Some(turn),
            "message.assistant",
            &reply(first + 4, first + 3, 0, false),
        ),
        event(
            first + 5,
            Some(turn),
            "turn.ended",
            r#"{"reason":"completed"}"#,
        ),
    ] {
        ledger.append(&event).unwrap();
    }
}

#[test]
fn a_session_watching_another_appends() {
    let mut ledger = watching_after(7);
    assert_eq!(waited(&ledger), [(id(B), at(5))], "从订的那一条的时刻算起");
    ledger.append(&notice(8, &session(B), B, "idle")).unwrap();
    assert!(waited(&ledger).is_empty(), "等到了，不再等");
    assert_eq!(ledger.next_seq().get(), 9);
}

/// 订的是这个会话自己，拒；同一条结果里有一个是自己的，整条不收。不知道自己是谁的账本（`Ledger::default()`）不查这一条。
#[test]
fn a_session_cannot_watch_itself() {
    let result = |effects: &[String]| {
        line(
            5,
            5,
            Some(3),
            "tool.result",
            KERNEL,
            &watched("call_4_1", effects),
        )
    };
    let mut ledger = watching_after(4);
    refused(
        &mut ledger,
        &result(&[watch(OWN)]),
        &format!("session {OWN} is this session: a session cannot watch itself"),
    );
    refused(
        &mut ledger,
        &result(&[watch(B), watch(OWN)]),
        "a session cannot watch itself",
    );
    ledger.append(&result(&[watch(C), watch(B)])).unwrap();
    assert_eq!(waited(&ledger), [(id(B), at(5)), (id(C), at(5))], "照编号");
    let mut blind = Ledger::default();
    for event in &opening()[..4] {
        blind.append(event).unwrap();
    }
    blind.append(&result(&[watch(OWN)])).unwrap();
}

/// 只认在等的：没订过的、订的是别的会话的、已经等到过的，都拒。订它的那一轮还在进行时到的照收：通知不带回合编号。
#[test]
fn a_notice_is_taken_only_while_watching() {
    let why = |session: &str| {
        format!(
            "session {session} is not being watched: never watched, the watching turn was undone, or its notice already came"
        )
    };
    let mut ledger = watching_after(4);
    refused(&mut ledger, &notice(5, &session(B), B, "idle"), &why(B));
    let mut ledger = watching_after(5);
    refused(&mut ledger, &notice(6, &session(C), C, "idle"), &why(C));
    ledger.append(&notice(6, &session(B), B, "idle")).unwrap();
    refused(&mut ledger, &notice(7, &session(B), B, "idle"), &why(B));
    refused(&mut ledger, &notice(7, KERNEL, B, "expired"), &why(B));
}

/// `by` 对得上：`idle` 的是那个会话，`expired`、`gone` 的是内核。不认识的原因是新版本才有的，不查 `by`。哪一种都算等到了头。
#[test]
fn a_notice_is_by_the_session_or_by_the_kernel() {
    let by_b = format!("session {B}");
    for (by, reason, who) in [
        (KERNEL.to_string(), "idle", by_b.as_str()),
        (session(C), "idle", &by_b),
        (ALICE.to_string(), "idle", &by_b),
        (session(B), "expired", "the kernel"),
        (session(B), "gone", "the kernel"),
        (ALICE.to_string(), "gone", "the kernel"),
    ] {
        refused(
            &mut watching_after(7),
            &notice(8, &by, B, reason),
            &format!("peer.idle for session {B} with reason {reason} should be by {who}"),
        );
    }
    for (by, reason) in [
        (session(B), "idle"),
        (KERNEL.to_string(), "expired"),
        (KERNEL.to_string(), "gone"),
        (ALICE.to_string(), "cancelled"),
        (session(C), "cancelled"),
    ] {
        let mut ledger = watching_after(7);
        ledger.append(&notice(8, &by, B, reason)).unwrap();
        assert!(waited(&ledger).is_empty(), "{reason}：等到了头");
    }
}

/// 撤掉订它的那一轮，就不算在等了；恢复了又算，还从原来的时刻算起。撤了以后开了新的一轮（恢复不了了），就一直不算。
#[test]
fn undoing_the_watching_turn_stops_the_watch() {
    let mut ledger = watching_after(7);
    ledger
        .append(&event(8, None, "turn.reverted", r#"{"turns":[3]}"#))
        .unwrap();
    assert!(waited(&ledger).is_empty());
    refused(
        &mut ledger,
        &notice(9, &session(B), B, "idle"),
        "is not being watched",
    );
    let mut restored = ledger.clone();
    restored
        .append(&event(9, None, "turn.unreverted", r#"{"turns":[3]}"#))
        .unwrap();
    assert_eq!(waited(&restored), [(id(B), at(5))]);
    restored
        .append(&notice(10, &session(B), B, "idle"))
        .unwrap();
    ledger
        .append(&event(9, None, "message.user", SAID))
        .unwrap();
    ledger
        .append(&event(10, Some(10), "turn.started", r#"{"trigger":9}"#))
        .unwrap();
    assert!(waited(&ledger).is_empty(), "撤掉的恢复不了了");
}

/// 又订了一次，从新的时刻算；撤掉又订的那一轮，回到前一次的时刻。等到过以后再订的从新算，撤掉它就不在等了：前一次的
/// 已经等到过。
#[test]
fn watching_again_counts_from_the_new_moment() {
    let mut ledger = watching_after(7);
    watch_again(&mut ledger, 8);
    assert_eq!(waited(&ledger), [(id(B), at(11))]);
    ledger
        .append(&event(14, None, "turn.reverted", r#"{"turns":[9]}"#))
        .unwrap();
    assert_eq!(waited(&ledger), [(id(B), at(5))], "回到第 3 轮订的");
    ledger.append(&notice(15, &session(B), B, "idle")).unwrap();
    watch_again(&mut ledger, 16);
    assert_eq!(waited(&ledger), [(id(B), at(19))]);
    ledger
        .append(&event(22, None, "turn.reverted", r#"{"turns":[17]}"#))
        .unwrap();
    assert!(waited(&ledger).is_empty(), "第 3 轮订的已经等到过了");
}
