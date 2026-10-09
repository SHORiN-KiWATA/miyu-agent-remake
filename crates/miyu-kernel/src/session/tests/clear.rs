//! 清空上下文的命令这一层（`docs/blueprint/compaction.md` 第十四条，施工 6-8 补）：空闲时收，单开一轮、同一批写空的检查点
//! 和结束，不请求模型、不注入、不跑回合开始的挂接点，都落了盘才回应；有回合在进行、上下文本来就是空的拒绝，什么都不追加。
//! 一整轮前后怎么走（下一轮、撤销、暂停、载入）在 `scenario/clear.rs`。

use super::load::Logged;
use super::*;
use crate::event::{CompactTrigger, ContextCompacted, EndReason, TurnEnded, TurnStarted};

/// 说过一轮的会话：1 造会话、2 hi、3 开回合、4、5 两块事实、6 回复、7 模型调用、8 结束。
fn after_one_turn() -> Session {
    let mut logged = Logged::new();
    let seen = logged.ask(2, "hi");
    logged.say(seen, "好。");
    assert_eq!(logged.last(), 8);
    logged.session
}

/// 编号是 `n` 的命令：alice 要清空上下文。
pub(super) fn clear(n: u64) -> Input {
    Input::Command(Received {
        id: id(n),
        by: alice(),
        at: at(n % 60),
        command: Command::Clear,
    })
}

fn rejected_reply(n: u64, reason: Reason) -> Action {
    Action::Reply {
        id: id(n),
        outcome: Outcome::Rejected {
            reason,
            about: Vec::new(),
        },
    }
}

#[test]
fn a_clear_writes_an_empty_checkpoint_in_a_turn_of_its_own() {
    let mut session = after_one_turn();
    let actions = session.handle(clear(3));
    let [Action::Append(events)] = actions.as_slice() else {
        panic!("只追加，同一批：{actions:?}");
    };
    let turn = Some(TurnId::new(seq(9)));
    assert_eq!(
        events.iter().map(|event| event.seq).collect::<Vec<_>>(),
        seqs(&[9, 10, 11])
    );
    for event in events {
        assert_eq!(event.turn, turn);
        assert_eq!(event.by, By::Kernel);
        assert_eq!(event.cause, Some(id(3)));
        assert_eq!(event.at, at(3));
    }
    assert_eq!(
        events[0].body,
        Body::TurnStarted(TurnStarted {
            trigger: None,
            triggers: Vec::new(),
            cwd: Some("~/src/miyu".to_string()),
            dirs: Vec::new(),
        })
    );
    assert_eq!(
        events[1].body,
        Body::ContextCompacted(ContextCompacted {
            upto: seq(8),
            summary: String::new(),
            trigger: Some(CompactTrigger::Clear),
            instructions: None,
            notes: String::new(),
            restored: Vec::new(),
            refills: None,
        })
    );
    assert_eq!(
        events[2].body,
        Body::TurnEnded(TurnEnded {
            reason: EndReason::Completed
        })
    );
    assert!(!session.idle(), "结束那一条落了盘才算空闲");

    let actions = session.handle(stored(11));
    assert_eq!(replies(&actions), [&accepted_reply(3, &[9])]);
    assert!(
        actions.iter().any(|action| *action
            == Action::RunTurnEndHooks {
                turn: TurnId::new(seq(9))
            }),
        "回合结束的挂接点照常跑：{actions:?}"
    );
    assert!(hooks(&actions).is_empty(), "不跑回合开始的挂接点");
    assert!(calls(&actions).is_empty(), "不请求模型");
    assert!(session.idle());
}

#[test]
fn a_running_turn_refuses_a_clear() {
    let mut session = after_one_turn();
    session.handle(send(3, "again"));
    let actions = session.handle(clear(4));
    assert_eq!(actions, [rejected_reply(4, Reason::TurnRunning)]);
}

#[test]
fn an_empty_context_refuses_a_clear() {
    // 还没说过话。
    let mut session = session();
    assert_eq!(
        session.handle(clear(3)),
        [rejected_reply(3, Reason::NothingToClear)]
    );
    // 刚清空过：检查点后面只有那一轮的开头结尾。
    let mut session = after_one_turn();
    session.handle(clear(3));
    session.handle(stored(11));
    assert_eq!(
        session.handle(clear(4)),
        [rejected_reply(4, Reason::NothingToClear)]
    );
    // 清空以后又说了一句：有得清。
    let events = appended(&session.handle(send(5, "again")));
    assert_eq!(events.first(), Some(&seq(12)));
    session.handle(stored(15));
    session.handle(interrupt(6));
    session.handle(stored(16));
    assert_eq!(appended(&session.handle(clear(7))), seqs(&[17, 18, 19]));
}

#[test]
fn the_same_clear_sent_again_is_answered_again_and_not_redone() {
    let mut session = after_one_turn();
    session.handle(clear(3));
    assert!(
        session.handle(clear(3)).is_empty(),
        "还没落盘，等落了盘再回"
    );
    let actions = session.handle(stored(11));
    assert_eq!(
        replies(&actions),
        [&accepted_reply(3, &[9]), &accepted_reply(3, &[9])]
    );
    assert_eq!(session.handle(clear(3)), [accepted_reply(3, &[9])]);
}
