//! 空不空闲（施工 3-9 上）：没有在跑的回合，也没有结束了、`turn.ended` 还没落盘的。核心看它决定能不能
//! 空闲退出。

use super::executor::*;
use super::*;

#[test]
fn a_session_is_idle_only_without_an_open_or_closing_turn() {
    let mut session = session();
    assert!(session.idle(), "刚造出来，没有回合");
    session.handle(send(1, "hi"));
    assert!(!session.idle(), "开了回合");
    session.handle(stored(5));
    session.handle(hooks_done(turn3(), Vec::new()));
    answer(&mut session, 5, "你好");
    assert!(!session.idle(), "回合结束了，turn.ended 还没落盘");
    session.handle(stored(8));
    assert!(session.idle(), "turn.ended 落了盘");
}
