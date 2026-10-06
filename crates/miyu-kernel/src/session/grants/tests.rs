//! 本会话放行过的怎么记（施工 D-1）：只认「本会话都允许」；没提规则的请求不算；一样的规则只记一次；照先后。

use super::Grants;
use crate::event::{ApprovalDecided, ApprovalRequested, Body, Decision, Event};
use crate::id::{CallId, Seq};
use crate::origin::By;
use crate::raw::RawJson;
use crate::time::Timestamp;
use crate::tool::Access;

fn raw(text: &str) -> RawJson {
    serde_json::from_str(text).unwrap()
}

fn event(n: u64, body: Body) -> Event {
    Event {
        seq: Seq::new(n).unwrap(),
        at: Timestamp::parse("2026-10-07T07:00:00.000Z").unwrap(),
        turn: None,
        by: By::Kernel,
        cause: None,
        body,
    }
}

fn call(n: u64) -> CallId {
    CallId::new(Seq::new(n).unwrap(), 1).unwrap()
}

fn asked(n: u64, rule: Option<&str>) -> Event {
    event(
        n,
        Body::ApprovalRequested(ApprovalRequested {
            call_id: call(n),
            access: Access::Write,
            rule: rule.map(raw),
            detail: None,
        }),
    )
}

fn decided(n: u64, asked: u64, decision: Decision) -> Event {
    event(
        n,
        Body::ApprovalDecided(ApprovalDecided {
            call_id: call(asked),
            decision,
            reason: None,
        }),
    )
}

fn noted(events: &[Event]) -> Vec<String> {
    let mut grants = Grants::default();
    for event in events {
        grants.note(event);
    }
    grants
        .granted()
        .iter()
        .map(|rule| rule.get().to_string())
        .collect()
}

#[test]
fn only_allowing_for_the_session_is_remembered() {
    let a = r#"{"tool":"write","write":["/a"]}"#;
    assert_eq!(
        noted(&[asked(1, Some(a)), decided(2, 1, Decision::Session)]),
        [a]
    );
    for decision in [
        Decision::Once,
        Decision::Deny,
        Decision::Workspace,
        Decision::Other("later".to_string()),
    ] {
        let label = format!("{decision:?}");
        assert!(
            noted(&[asked(1, Some(a)), decided(2, 1, decision)]).is_empty(),
            "{label}"
        );
    }
}

#[test]
fn a_request_without_a_rule_or_without_a_decision_grants_nothing() {
    assert!(noted(&[asked(1, None), decided(2, 1, Decision::Session)]).is_empty());
    assert!(noted(&[asked(1, Some(r#"{"write":["/a"]}"#))]).is_empty());
    // 决定对不上请求的不算。
    assert!(
        noted(&[
            asked(1, Some(r#"{"write":["/a"]}"#)),
            decided(2, 9, Decision::Session)
        ])
        .is_empty()
    );
}

#[test]
fn rules_are_kept_in_order_and_each_once() {
    let (a, b) = (r#"{"write":["/a"]}"#, r#"{"write":["/b"]}"#);
    let events = [
        asked(1, Some(a)),
        asked(2, Some(b)),
        decided(3, 2, Decision::Session),
        decided(4, 1, Decision::Session),
        asked(5, Some(b)),
        decided(6, 5, Decision::Session),
    ];
    assert_eq!(noted(&events), [b, a]);
}
