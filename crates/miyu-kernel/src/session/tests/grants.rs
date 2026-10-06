//! 本会话放行过的（施工 D-1，`docs/blueprint/kernel/asking.md`「本会话放行过的」）：人选了「本会话都允许」，以后交给链的
//! 都带上那条规则；只认这一项；载入以后照日志算回来；撤销不收回。

use super::approval::{answer as decide, permissions, raw};
use super::executor::*;
use super::load::{Logged, load};
use super::revert::revert;
use super::*;
use crate::event::Decision;
use crate::raw::RawJson;
use crate::tool::Access;

const RULE: &str = r#"{"tool":"write","write":["/home/alice/notes"]}"#;

/// 请求 `seen` 调一件写文件的，链说要问人、提了规则；人照 `decision` 回答（命令 `n`），都落了盘、跑完了。交回下一次请求
/// 的 `seen`。
fn asked_and_answered(logged: &mut Logged, seen: u64, n: u64, decision: Decision) -> u64 {
    let actions = call_tools(&mut logged.session, seen, &[("write", "{}")]);
    logged.log.extend(appended_events(&actions));
    let actions = logged.handle(stored(logged.last()));
    let call_id = guards(&actions)[0];
    let verdict = Verdict::Ask {
        module: permissions(),
        access: Access::Write,
        rule: Some(raw(RULE)),
        detail: None,
    };
    logged.handle(guarded(call_id, verdict));
    logged.handle(stored(logged.last()));
    logged.handle(decide(n, call_id, decision, None));
    let actions = logged.handle(stored(logged.last()));
    let mut next = calls(&actions);
    if let Some(&running) = ran(&actions).first() {
        logged.handle(done(running, "ok"));
        next = calls(&logged.handle(stored(logged.last())));
    }
    next.remove(0).0.get()
}

/// 请求 `seen` 又调一件写文件的、落了盘：交给链的那一次带着的放行规则。
fn next_grants(logged: &mut Logged, seen: u64) -> Vec<RawJson> {
    let actions = call_tools(&mut logged.session, seen, &[("write", "{}")]);
    logged.log.extend(appended_events(&actions));
    let actions = logged.handle(stored(logged.last()));
    match actions.last() {
        Some(Action::GuardTool { grants, .. }) => grants.clone(),
        other => panic!("应该交给链：{other:?}"),
    }
}

#[test]
fn allowing_for_the_session_is_handed_to_the_chain_from_then_on() {
    let mut logged = Logged::new();
    let seen = logged.ask(1, "记一下");
    let seen = asked_and_answered(&mut logged, seen, 2, Decision::Session);
    assert_eq!(next_grants(&mut logged, seen), [raw(RULE)]);
}

#[test]
fn allowing_once_or_denying_grants_nothing() {
    for decision in [Decision::Once, Decision::Deny] {
        let label = format!("{decision:?}");
        let mut logged = Logged::new();
        let seen = logged.ask(1, "记一下");
        let seen = asked_and_answered(&mut logged, seen, 2, decision);
        assert!(next_grants(&mut logged, seen).is_empty(), "{label}");
    }
}

#[test]
fn a_loaded_session_remembers_and_undoing_does_not_take_it_back() {
    let mut logged = Logged::new();
    let seen = logged.ask(1, "记一下");
    let seen = asked_and_answered(&mut logged, seen, 2, Decision::Session);
    logged.say(seen, "记好了");
    // 载入：照日志算回来。
    let (session, _) = load(logged.log.clone());
    let mut loaded = Logged {
        session,
        log: logged.log.clone(),
    };
    let seen = loaded.ask(3, "再记一下");
    assert_eq!(next_grants(&mut loaded, seen), [raw(RULE)]);
    // 撤掉放行的那一轮：放行还在。
    let mut logged = Logged::new();
    let seen = logged.ask(1, "记一下");
    let seen = asked_and_answered(&mut logged, seen, 2, Decision::Session);
    logged.say(seen, "记好了");
    logged.handle(revert(3, 3));
    logged.handle(stored(logged.last()));
    let seen = logged.ask(4, "重新来");
    assert_eq!(next_grants(&mut logged, seen), [raw(RULE)]);
}
