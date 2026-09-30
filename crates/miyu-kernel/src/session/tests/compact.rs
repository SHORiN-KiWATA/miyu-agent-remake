//! 手动压缩的命令这一层（`docs/blueprint/compaction.md` 第七条，施工 6-8）：空闲时收，单开一轮（没有触发、不注入事实、
//! 不跑回合开始的挂接点），它落了盘才回应、接着发摘要请求；有回合在进行、没有能压的拒绝，什么都不追加。一整轮怎么走
//! 在 `scenario/manual.rs`。

use super::executor::deepseek;
use super::load::Logged;
use super::*;
use crate::event::TurnStarted;
use crate::session::Compaction;

/// 压缩的数：尾巴的上限是 `tail`，别的很小。
fn compaction(tail: u64) -> Compaction {
    Compaction {
        reserve_cap: 10,
        margin: 10,
        tail,
        price: crate::estimate::Flat {
            image: 50,
            file: 50,
        },
        rebuild: None,
        pause: None,
        shorten: None,
        isolate: false,
    }
}

/// 说过一轮的日志：1 造会话、2 hi、3 开回合、4、5 两块事实、6 回复、7 模型调用、8 结束。
fn one_turn_log() -> Vec<Event> {
    let mut logged = Logged::new();
    let seen = logged.ask(2, "hi");
    logged.say(seen, "好。");
    assert_eq!(logged.last(), 8);
    logged.log
}

/// 照 `compaction` 载入这份日志；`window` 是交的窗口，没有的不交限额。
fn loaded(log: Vec<Event>, compaction: Option<Compaction>, window: Option<u64>) -> Session {
    let mut policy = policy();
    policy.compaction = compaction;
    let (mut session, actions) =
        Session::load(session_id(), log, at(55), policy, environment("~/src/miyu")).unwrap();
    assert!(actions.is_empty(), "走完了的会话载入不补什么");
    if let Some(window) = window {
        session.handle(Input::Limits(Limits {
            model: deepseek(),
            window: Some(window),
            max_output: None,
            images: None,
        }));
    }
    session
}

/// 说过一轮、会压缩、尾巴不留、窗口大到不会自动压的会话。
fn after_one_turn() -> Session {
    loaded(one_turn_log(), Some(compaction(0)), Some(100_000))
}

/// 编号是 `n` 的命令：alice 要手动压缩，附上 `instructions`。
pub(super) fn compact(n: u64, instructions: Option<&str>) -> Input {
    Input::Command(Received {
        id: id(n),
        by: alice(),
        at: at(n % 60),
        command: Command::Compact {
            instructions: instructions.map(str::to_string),
        },
    })
}

fn rejected_reply(n: u64, reason: Reason) -> Action {
    Action::Reply {
        id: id(n),
        outcome: Outcome::Rejected { reason },
    }
}

#[test]
fn a_manual_compaction_opens_a_turn_of_its_own_and_answers_once_it_is_stored() {
    let mut session = after_one_turn();
    let actions = session.handle(compact(3, Some("keep the plan")));
    let events = appended_events(&actions);
    assert_eq!(events.len(), 1, "只有回合开始，不追加事实：{events:?}");
    let started = &events[0];
    assert_eq!(started.seq, seq(9));
    assert_eq!(started.turn, Some(TurnId::new(seq(9))));
    assert_eq!(started.by, By::Kernel);
    assert_eq!(started.cause, Some(id(3)));
    assert_eq!(
        started.body,
        Body::TurnStarted(TurnStarted {
            trigger: None,
            cwd: Some("~/src/miyu".to_string()),
            dirs: Vec::new(),
        })
    );
    assert!(replies(&actions).is_empty(), "落了盘才回应");
    assert!(!session.idle(), "那一轮在跑");

    let actions = session.handle(stored(9));
    assert_eq!(replies(&actions), [&accepted_reply(3, &[9])]);
    assert!(hooks(&actions).is_empty(), "不跑回合开始的挂接点");
    assert!(appended(&actions).is_empty(), "不查事实");
    // 摘要请求：替代到收下命令时落了盘的最后一条，要求在指令前面一条。
    assert_eq!(
        calls(&actions),
        [(
            seq(8),
            listing(&one_turn_log()) + "instructions: keep the plan\nsummarize\n"
        )]
    );
}

#[test]
fn instructions_of_only_blanks_are_no_instructions() {
    let mut session = after_one_turn();
    session.handle(compact(3, Some(" \n\t ")));
    let actions = session.handle(stored(9));
    assert_eq!(
        calls(&actions),
        [(seq(8), listing(&one_turn_log()) + "summarize\n")]
    );
}

#[test]
fn a_running_turn_refuses_a_manual_compaction() {
    let mut session = after_one_turn();
    session.handle(send(3, "again"));
    let actions = session.handle(compact(4, None));
    assert_eq!(actions, [rejected_reply(4, Reason::TurnRunning)]);
}

#[test]
fn a_session_with_nothing_to_compact_refuses_it() {
    // 还没说过话。
    let fresh = Logged::new().log;
    let mut session = loaded(fresh, Some(compaction(0)), Some(100_000));
    let actions = session.handle(compact(3, None));
    assert_eq!(actions, [rejected_reply(3, Reason::NothingToCompact)]);
    // 说过的全在尾巴里。
    let mut session = loaded(one_turn_log(), Some(compaction(1000)), Some(100_000));
    let actions = session.handle(compact(3, None));
    assert_eq!(actions, [rejected_reply(3, Reason::NothingToCompact)]);
    // 执行器没交过限额：算不了尾巴。
    let mut session = loaded(one_turn_log(), Some(compaction(0)), None);
    let actions = session.handle(compact(3, None));
    assert_eq!(actions, [rejected_reply(3, Reason::NothingToCompact)]);
    // 6-2 以前造的快照：策略里没有压缩。
    let mut session = loaded(one_turn_log(), None, Some(100_000));
    let actions = session.handle(compact(3, None));
    assert_eq!(actions, [rejected_reply(3, Reason::NothingToCompact)]);
}

#[test]
fn a_model_without_a_window_can_still_be_compacted_by_hand() {
    // 没报窗口：不主动压，手动的照样压，尾巴的预算是策略的上限。
    let mut session = loaded(one_turn_log(), Some(compaction(0)), None);
    session.handle(Input::Limits(Limits {
        model: deepseek(),
        window: None,
        max_output: None,
        images: None,
    }));
    session.handle(compact(3, None));
    let actions = session.handle(stored(9));
    assert_eq!(calls(&actions).len(), 1, "发了摘要请求");
}

#[test]
fn the_same_compaction_sent_again_is_answered_again_and_not_redone() {
    let mut session = after_one_turn();
    session.handle(compact(3, None));
    session.handle(stored(9));
    let actions = session.handle(compact(3, None));
    assert_eq!(actions, [accepted_reply(3, &[9])]);
}

/// 尾巴的预算（`compaction.md` 第七条第 2 条）：有压缩线的，不超过它的四分之一；没报窗口的，就是策略的上限。
#[test]
fn the_tail_is_the_smaller_of_its_cap_and_a_quarter_of_the_line() {
    // 上限 1000、压缩线 40：预算是 10，第一句那一组（带着两块事实）放不进尾巴，压得掉。
    let mut session = loaded(one_turn_log(), Some(compaction(1000)), Some(60));
    let actions = session.handle(compact(3, None));
    assert_eq!(appended(&actions), seqs(&[9]), "{actions:?}");
    // 没报窗口：预算是上限 1000，说过的全在尾巴里，没有能压的。
    let mut session = loaded(one_turn_log(), Some(compaction(1000)), None);
    session.handle(Input::Limits(Limits {
        model: deepseek(),
        window: None,
        max_output: None,
        images: None,
    }));
    let actions = session.handle(compact(3, None));
    assert_eq!(actions, [rejected_reply(3, Reason::NothingToCompact)]);
}
