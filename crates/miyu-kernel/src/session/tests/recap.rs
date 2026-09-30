//! 回顾（施工 3-8 四补，`docs/blueprint/kernel/session.md`「回顾」）的这一层：照落了盘的算照到哪；那一句落了盘才回；对不上
//! 在路上的那一次的回报不理；发出去了报两次只认第一次；还没发出去就来增量、没发出去就说完了、增量对不上的都没写成；回顾不记
//! 编号，同一个编号再来就是再要一次。整轮怎么走在 `scenario/recap.rs`。

use super::executor::deepseek;
use super::*;
use crate::accumulate::{Delta, Kind};
use crate::event::{CallError, ErrorClass, ModelCalled};
use crate::id::ContentHash;
use crate::testkit::{Line, Stage};

/// 一轮说完了的会话，照替身的日志载入（1 到 8，6 是回复），日志都落了盘。
fn answered() -> Session {
    loaded(answered_log())
}

/// 一轮说完了的日志：替身跑的。
fn answered_log() -> Vec<Event> {
    let mut stage = Stage::new(policy, environment("~/src/miyu"), at(0));
    stage.model([Line::says("好。")]);
    stage.say("hi");
    stage.log().to_vec()
}

/// 照日志 `log` 载入的会话。
fn loaded(log: Vec<Event>) -> Session {
    let (session, _) = Session::load(
        session_id(),
        log,
        at(30),
        policy(),
        environment("~/src/miyu"),
    )
    .unwrap();
    session
}

/// 要一句回顾：命令编号是 `n`。
fn recap(n: u64) -> Input {
    Input::Command(Received {
        id: id(n),
        by: alice(),
        at: at(31),
        command: Command::Recap,
    })
}

/// 回顾 `upto` 发出去了，在 07:00:`second`。
fn sent(upto: u64, second: u64) -> Input {
    Input::RecapSent {
        at: at(second),
        upto: seq(upto),
        model: deepseek(),
        request: ContentHash::of(b"recap"),
    }
}

/// 回顾 `upto` 的一段增量。
fn delta(upto: u64, delta: Delta) -> Input {
    Input::RecapDelta {
        at: at(42),
        upto: seq(upto),
        delta,
    }
}

/// 回顾 `upto` 的一块正文：07:00:42 开始，07:00:43 来字。
fn said(upto: u64, text: &str) -> [Input; 2] {
    let text = Input::RecapDelta {
        at: at(43),
        upto: seq(upto),
        delta: Delta::Text {
            index: 0,
            text: text.to_string(),
        },
    };
    let start = Delta::Start {
        index: 0,
        kind: Kind::Text,
    };
    [delta(upto, start), text]
}

/// 回顾 `upto` 在 07:00:45 说完了；出错的带上分类。
fn ended(upto: u64, error: Option<ErrorClass>) -> Input {
    Input::RecapEnded {
        at: at(45),
        upto: seq(upto),
        usage: None,
        error: error.map(|class| CallError {
            class,
            message: "503".to_string(),
            status: None,
        }),
    }
}

/// 送一串回顾的回报进会话。
type Feed = fn(&mut Session);

/// 追加的回顾的 `model.called`。
fn called(actions: &[Action]) -> ModelCalled {
    appended_events(actions)
        .into_iter()
        .find_map(|event| match event.body {
            Body::ModelCalled(called) => Some(called),
            _ => None,
        })
        .expect("记了一条 model.called")
}

#[test]
fn it_goes_by_what_is_stored_and_answers_once_the_line_is_stored() {
    let mut session = answered();
    // 又说了一句，开了一轮，还没落盘：回顾照落了盘的，照到第 6 条。
    let opened = session.handle(send(20, "again"));
    assert_eq!(appended(&opened), seqs(&[9, 10]));
    let asked = session.handle(recap(21));
    let [Action::Recap { upto, request }] = asked.as_slice() else {
        panic!("交出一次回顾：{asked:?}");
    };
    assert_eq!(*upto, seq(6));
    assert_eq!(
        listed_request(request),
        "2 message.user\n6 message.assistant\nrecap\n"
    );
    assert!(session.handle(sent(6, 40)).is_empty());
    for input in said(6, " 在打招呼。") {
        assert!(session.handle(input).is_empty(), "增量不推给头");
    }
    let done = session.handle(ended(6, None));
    assert_eq!(appended(&done), seqs(&[11, 12]));
    assert!(replies(&done).is_empty(), "那一句落了盘才回");
    let partly = session.handle(stored(11));
    assert!(
        replies(&partly)
            .iter()
            .all(|reply| !matches!(reply, Action::Reply { id: replied, .. } if *replied == id(21))),
        "只落到 model.called 还不回"
    );
    let whole = session.handle(stored(12));
    assert!(whole.contains(&Action::Reply {
        id: id(21),
        outcome: Outcome::Recapped {
            text: "在打招呼。".to_string(),
            upto: seq(6),
            cached: false,
        },
    }));
}

#[test]
fn reports_for_another_recap_are_ignored_and_the_first_sent_counts() {
    let mut session = answered();
    session.handle(recap(20));
    let early = Delta::Start {
        index: 0,
        kind: Kind::Text,
    };
    for stale in [sent(5, 38), delta(5, early), ended(5, None)] {
        assert!(session.handle(stale).is_empty(), "对不上的不理");
    }
    session.handle(sent(6, 40));
    session.handle(sent(6, 43));
    for input in said(6, "在打招呼。") {
        session.handle(input);
    }
    let done = session.handle(ended(6, None));
    let called = called(&done);
    assert_eq!(called.error, None, "对不上的增量没收进来");
    assert_eq!(
        called.duration_ms,
        Some(5_000),
        "从第一次报发出去的算起，对不上的那一次不算"
    );
    assert_eq!(called.first_token_ms, Some(2_000), "第一段增量到的时刻");
    assert_eq!(called.endpoint, Some(deepseek().endpoint));
}

#[test]
fn a_broken_stream_is_not_written() {
    // 还没报发出去就来了增量；没报发出去就说完了；增量对不上（后面的也不收）。
    let early = |session: &mut Session| {
        session.handle(said(6, "早了")[0].clone());
        session.handle(sent(6, 40));
        for input in said(6, "在打招呼。") {
            session.handle(input);
        }
    };
    let unsent = |_: &mut Session| {};
    let mismatched = |session: &mut Session| {
        session.handle(sent(6, 40));
        session.handle(delta(6, Delta::End { index: 3 }));
        // 头一个错算数，后面的增量不收。
        session.handle(delta(6, Delta::End { index: 5 }));
        for input in said(6, "在打招呼。") {
            session.handle(input);
        }
    };
    let cases: [(Feed, &str); 3] = [
        (early, "请求还没发出去就来了增量"),
        (unsent, "请求还没发出去就说完了"),
        (mismatched, "模型的增量对不上，第 3 块：这一块还没开始"),
    ];
    for (feed, message) in cases {
        let mut session = answered();
        session.handle(recap(20));
        feed(&mut session);
        let done = session.handle(ended(6, None));
        let called = called(&done);
        let error = called.error.expect("没写成");
        assert_eq!(
            (error.class, error.message.as_str()),
            (ErrorClass::BadStream, message)
        );
        assert_eq!(appended(&done).len(), 1, "没有那一句");
        let landed = session.handle(stored(9));
        assert!(landed.contains(&Action::Reply {
            id: id(20),
            outcome: Outcome::Rejected {
                reason: Reason::RecapFailed
            },
        }));
    }
}

/// 回顾不记编号：同一个编号再来就是再要一次，没有新内容的交回上一句。别的命令照旧只生效一次。
#[test]
fn the_same_id_asks_again() {
    let mut session = answered();
    session.handle(recap(20));
    session.handle(sent(6, 40));
    for input in said(6, "在打招呼。") {
        session.handle(input);
    }
    session.handle(ended(6, Some(ErrorClass::Retryable)));
    session.handle(stored(9));
    let again = session.handle(recap(20));
    assert!(
        matches!(again.as_slice(), [Action::Recap { upto, .. }] if *upto == seq(6)),
        "没写成的再要一次，照样请求：{again:?}"
    );
}

/// 载入时照日志认回接受过的编号（`cause`）：回顾的那两条也带着它。回顾不查，同一个编号再来还是要一句回顾，交回上一句，
/// 不当成接受过的命令照上一次回 `Accepted`。
#[test]
fn after_a_reload_the_same_id_still_asks_for_a_recap() {
    let mut log = answered_log();
    let mut session = loaded(log.clone());
    session.handle(recap(20));
    session.handle(sent(6, 40));
    for input in said(6, "在打招呼。") {
        session.handle(input);
    }
    log.extend(appended_events(&session.handle(ended(6, None))));
    let mut session = loaded(log);
    assert_eq!(
        session.handle(recap(20)),
        [Action::Reply {
            id: id(20),
            outcome: Outcome::Recapped {
                text: "在打招呼。".to_string(),
                upto: seq(6),
                cached: true,
            },
        }]
    );
}
