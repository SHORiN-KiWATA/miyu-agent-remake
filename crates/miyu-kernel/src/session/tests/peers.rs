//! 会话知道自己是谁（施工 C-1，`kernel/history.md`「账本」）：造的、载入的会话，账本都照它自己的编号查。订「空了告诉我」
//! 订的是它自己的，过不了账本：造的会话当场停下（内核自己造的事件过不了账本是内核的错），载入的日志拒绝。订别的会话的照收。

use super::executor::*;
use super::load::{Logged, load};
use super::*;
use crate::event::{Effect, PeerWatch};
use crate::id::CallId;
use crate::ledger::LedgerError;

/// 另一个会话。
const PEER: &str = "0192f3a0-2222-7abc-8def-5566778899aa";

/// 调用 `call_id` 执行完了，订了会话 `session`。
fn watched(call_id: CallId, session: SessionId) -> Input {
    let mut input = done(call_id, "ok");
    if let Input::ToolDone { effects, .. } = &mut input {
        effects.push(Effect::PeerWatch(PeerWatch { session }));
    }
    input
}

/// 一个会话说了一句，请求调了一件 `read`，执行完了订了会话 `session`：第 8 条是那条结果。
fn watching(session: SessionId) -> Logged {
    let mut logged = Logged::new();
    let seen = logged.ask(1, "hi");
    logged.tools(seen, &[("read", "{}")]);
    logged.allowing(watched(call(6, 1), session));
    logged
}

#[test]
fn a_session_can_watch_another_and_load_again() {
    let logged = watching(SessionId::parse(PEER).unwrap());
    assert!(matches!(
        &logged.log[7].body,
        Body::ToolResult(result) if matches!(result.effects.as_slice(), [Effect::PeerWatch(_)])
    ));
    let (loaded, _) = load(logged.log);
    assert_eq!(loaded.ledger.watching().count(), 1);
}

#[test]
#[should_panic(expected = "a session cannot watch itself")]
fn a_new_session_cannot_watch_itself() {
    watching(session_id());
}

#[test]
fn a_log_where_the_session_watches_itself_is_refused() {
    let mut log = watching(SessionId::parse(PEER).unwrap()).log;
    let Body::ToolResult(result) = &mut log[7].body else {
        panic!("第 8 条是那条结果：{:?}", log[7]);
    };
    result.effects = vec![Effect::PeerWatch(PeerWatch {
        session: session_id(),
    })];
    let Err(LoadError::Broken(LedgerError { seq: broken, why })) =
        Session::load(session_id(), log, at(55), policy(), environment("~"))
    else {
        panic!("订自己的日志应该拒绝");
    };
    assert_eq!(broken, seq(8));
    assert!(why.contains("a session cannot watch itself"), "{why}");
}
