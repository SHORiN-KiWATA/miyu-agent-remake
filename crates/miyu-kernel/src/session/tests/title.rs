//! 起标题（施工 3-8 五补，`docs/blueprint/kernel/session.md`「起标题」）的这一层：`turn.ended` 落了盘才起、照落了盘的有效
//! 历史组装；对不上在路上的那一次的回报不理（名字、用途都要对得上）；标题记下以前不回什么（没有命令等着它）。整轮怎么走在
//! `scenario/title.rs`，收回报的共用规矩在 `recap.rs` 测。

use super::executor::*;
use super::*;
use crate::accumulate::{Delta, Kind};
use crate::event::{MetaChanged, Purpose};
use crate::id::ContentHash;

/// 试两次、最多 50 个字的会话：说了一句，她答完了，`turn.ended`（8）还没落盘。
fn answered() -> Session {
    let mut session = session_with(Policy {
        titles: Some(Titles {
            tries: 2,
            chars: 50,
        }),
        ..policy()
    });
    session.handle(send(1, "hi"));
    session.handle(stored(5));
    session.handle(hooks_done(turn3(), Vec::new()));
    let done = answer(&mut session, 5, "好。");
    assert_eq!(appended(&done), seqs(&[6, 7, 8]));
    session
}

/// 起标题的一样回报：用途 `purpose`，名字 `upto`。
fn aside_sent(purpose: Purpose, upto: u64) -> Input {
    Input::AsideSent {
        at: at(40),
        purpose,
        upto: seq(upto),
        model: deepseek(),
        request: ContentHash::of(b"title"),
    }
}

/// 一段正文增量。
fn aside_delta(purpose: Purpose, upto: u64, delta: Delta) -> Input {
    Input::AsideDelta {
        at: at(41),
        purpose,
        upto: seq(upto),
        delta,
    }
}

/// 说完了。
fn aside_ended(purpose: Purpose, upto: u64) -> Input {
    Input::AsideEnded {
        at: at(45),
        purpose,
        upto: seq(upto),
        usage: None,
        cost: None,
        error: None,
    }
}

/// 说了 `text` 这一块正文。
fn aside_said(purpose: &Purpose, upto: u64, text: &str) -> Vec<Input> {
    [
        Delta::Start {
            index: 0,
            kind: Kind::Text,
        },
        Delta::Text {
            index: 0,
            text: text.to_string(),
        },
        Delta::End { index: 0 },
    ]
    .into_iter()
    .map(|delta| aside_delta(purpose.clone(), upto, delta))
    .collect()
}

#[test]
fn it_asks_once_the_turn_end_is_stored() {
    let mut session = answered();
    let early = session.handle(stored(7));
    assert!(
        !early
            .iter()
            .any(|action| matches!(action, Action::Aside { .. })),
        "turn.ended 还没落盘，不起：{early:?}"
    );
    let asked = session.handle(stored(8));
    let titles: Vec<(&Purpose, Seq, String)> = asked
        .iter()
        .filter_map(|action| match action {
            Action::Aside {
                purpose,
                upto,
                request,
            } => Some((purpose, *upto, listed_request(request))),
            _ => None,
        })
        .collect();
    assert_eq!(
        titles,
        [(
            &Purpose::Title,
            seq(6),
            "2 message.user\n6 message.assistant\ntitle\n".to_string()
        )]
    );
    assert!(
        asked.contains(&Action::RunTurnEndHooks { turn: turn3() }),
        "回合结束的挂接点照跑"
    );
}

#[test]
fn reports_for_another_request_are_ignored() {
    let mut session = answered();
    session.handle(stored(8));
    // 名字对不上、用途对不上（回顾那一路）、不认识的用途：都不理。
    for stale in [
        aside_ended(Purpose::Title, 5),
        aside_ended(Purpose::Recap, 6),
        aside_ended(Purpose::Other("vision".to_string()), 6),
        aside_sent(Purpose::Recap, 6),
    ] {
        assert!(session.handle(stale).is_empty(), "对不上的不理");
    }
    for purpose in [Purpose::Recap, Purpose::Other("vision".to_string())] {
        for stale in aside_said(&purpose, 6, "串进来的") {
            assert!(session.handle(stale).is_empty());
        }
    }
    session.handle(aside_sent(Purpose::Title, 6));
    for input in aside_said(&Purpose::Title, 6, "打招呼") {
        assert!(session.handle(input).is_empty(), "增量不推给头");
    }
    let done = session.handle(aside_ended(Purpose::Title, 6));
    let events = appended_events(&done);
    assert_eq!(appended(&done), seqs(&[9, 10]));
    assert_eq!(
        events[1].body,
        Body::MetaChanged(MetaChanged {
            title: Some("打招呼".to_string()),
            pinned: None,
        }),
        "对不上的增量没收进来"
    );
    assert!(replies(&done).is_empty(), "没有命令等着它");
    assert!(
        session.handle(aside_ended(Purpose::Title, 6)).is_empty(),
        "说完了的再报一次不理"
    );
}

/// 起标题的请求带着金额说完了（施工 8-15）：照样记进它的 `model.called`。
#[test]
fn the_title_request_keeps_its_cost() {
    use crate::event::{Cost, Prices, Real};
    let mut session = answered();
    session.handle(stored(8));
    session.handle(aside_sent(Purpose::Title, 6));
    for input in aside_said(&Purpose::Title, 6, "打招呼") {
        session.handle(input);
    }
    let cost = Cost {
        amount: Real::new(0.002),
        currency: "CNY".to_string(),
        price: Prices::default(),
        multiplier: Real::new(0.5),
        source: "config:system/config.toml:3".to_string(),
        above: None,
    };
    let done = session.handle(Input::AsideEnded {
        at: at(45),
        purpose: Purpose::Title,
        upto: seq(6),
        usage: Some(usage()),
        cost: Some(cost.clone()),
        error: None,
    });
    let called = appended_events(&done)
        .into_iter()
        .find_map(|event| match event.body {
            Body::ModelCalled(called) => Some(called),
            _ => None,
        })
        .expect("记了 model.called");
    assert_eq!(called.cost.as_deref(), Some(&cost));
}
