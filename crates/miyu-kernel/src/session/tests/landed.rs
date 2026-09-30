//! 落了盘的最后一条（施工 3-8 六补）：`Stored` 送进来、推送了才往前走；追加了还没落盘的不算。载入的是日志里最后一条。
//! 会话 actor 订阅时照它定补到哪一条（`docs/blueprint/session/actor.md` 第 6 条）。

use super::load::{Logged, load};
use super::*;

#[test]
fn stored_moves_only_when_events_land() {
    let created: SessionCreated = serde_json::from_str(CREATED).unwrap();
    let (mut session, _) = Session::create(
        session_id(),
        id(0),
        alice(),
        at(0),
        created,
        policy(),
        environment("~/src/miyu"),
    );
    assert_eq!(session.landed(), None, "造会话那一条还没落盘");
    session.handle(stored(1));
    assert_eq!(session.landed(), Some(seq(1)));
    session.handle(send(1, "hi"));
    assert_eq!(session.landed(), Some(seq(1)), "追加了，还没落盘");
    session.handle(stored(3));
    assert_eq!(session.landed(), Some(seq(3)), "落了一部分");
    session.handle(stored(5));
    assert_eq!(session.landed(), Some(seq(5)));
}

#[test]
fn a_loaded_session_has_stored_its_last_event() {
    let mut logged = Logged::new();
    logged.handle(send(1, "hi"));
    let last = logged.last();
    let (session, _) = load(logged.log);
    assert_eq!(session.landed(), Some(seq(last)));
}
