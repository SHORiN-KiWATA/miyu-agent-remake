//! 回复每一块的起止（施工 2-3 补，`kernel/session.md`「收回复」第 2 条、第 3 条第 3 款）：照增量到的时刻记，收块的
//! `End` 不算；流里的块和落盘的块对不上的，照落盘的；对不上的那一段不算；时钟往回拨了不往回挪。执行器替身整轮跑
//! 的在 `scenario/spans.rs`。

use super::executor::*;
use super::*;
use crate::accumulate::{Delta, Kind};
use crate::block::Private;
use crate::event::{BlockSpan, ErrorClass, ModelCalled};

/// 一块一项，从发出去算起的毫秒数：起、止。
fn spans(pairs: &[(u64, u64)]) -> Option<Vec<BlockSpan>> {
    Some(
        pairs
            .iter()
            .map(|&(start_ms, end_ms)| BlockSpan { start_ms, end_ms })
            .collect(),
    )
}

/// 请求 5 号在 07:00:40 发出去，照先后送进这几段增量：07:00 的第几秒到的、哪一段。
fn streamed(deltas: Vec<(u64, Delta)>) -> Session {
    let mut session = asking();
    session.handle(sent(5));
    for (second, piece) in deltas {
        session.handle(delta(5, second, piece));
    }
    session
}

/// 第 `index` 块开始，是 `kind`。
fn start(index: usize, kind: Kind) -> Delta {
    Delta::Start { index, kind }
}

/// 第 `index` 块的一段字。
fn text(index: usize, text: &str) -> Delta {
    Delta::Text {
        index,
        text: text.to_string(),
    }
}

/// 第 `index` 块收全了。
fn end(index: usize) -> Delta {
    Delta::End { index }
}

/// 调 `read` 的一块。
fn read() -> Kind {
    Kind::ToolCall {
        name: "read".to_string(),
    }
}

/// 这一步写的回复（没有的是空的）和 `model.called`。
fn written(actions: &[Action]) -> (Vec<Block>, ModelCalled) {
    let events = appended_events(actions);
    let reply = events.iter().find_map(|event| match &event.body {
        Body::MessageAssistant(reply) => Some(reply.blocks.clone()),
        _ => None,
    });
    let called = events
        .iter()
        .find(|event| matches!(event.body, Body::ModelCalled(_)))
        .map(called_of)
        .expect("每次请求都记一条 model.called")
        .clone();
    (reply.unwrap_or_default(), called)
}

#[test]
fn each_block_runs_from_its_first_delta_to_its_last_and_the_end_marks_do_not_count() {
    // 照驱动的样子：字交错着来，私有数据跟在思考的字后面，流完了才一起收块（07:00:45）。
    let private: Private =
        serde_json::from_str(r#"{"driver":"anthropic","data":{"signature":"sig"}}"#).unwrap();
    let mut session = streamed(vec![
        (41, start(0, Kind::Reasoning)),
        (41, text(0, "想")),
        (42, text(0, "想好了")),
        (42, start(1, Kind::Text)),
        (42, text(1, "我")),
        (43, Delta::Private { index: 0, private }),
        (43, start(2, read())),
        (43, text(2, r#"{"path":"#)),
        (44, text(1, "看看")),
        (44, text(2, r#""src"}"#)),
        (45, end(0)),
        (45, end(1)),
        (45, end(2)),
    ]);
    let (reply, called) = written(&session.handle(ended(5)));
    assert_eq!(reply.len(), 3);
    assert_eq!(called.first_token_ms, Some(1000));
    assert_eq!(called.duration_ms, Some(5000));
    assert_eq!(
        called.blocks,
        spans(&[(1000, 3000), (2000, 4000), (3000, 4000)])
    );
}

#[test]
fn blocks_left_out_of_the_reply_are_left_out_of_the_times() {
    // 空的思考、空的正文不写进回复，也不记；后面的照落盘的先后对上。
    let mut session = streamed(vec![
        (41, start(0, Kind::Reasoning)),
        (42, start(1, Kind::Text)),
        (42, text(1, "好")),
        (43, start(2, Kind::Text)),
        (44, start(3, read())),
        (44, text(3, "{}")),
        (44, end(0)),
        (44, end(1)),
        (44, end(2)),
        (44, end(3)),
    ]);
    let (reply, called) = written(&session.handle(ended(5)));
    assert_eq!(reply.len(), 2, "{reply:?}");
    assert_eq!(called.blocks, spans(&[(2000, 2000), (4000, 4000)]));
}

#[test]
fn a_broken_reply_times_only_what_it_keeps() {
    // 出错断了：没收全的调用丢掉，收全了的也去掉，只记留下的思考和正文。
    let mut session = streamed(vec![
        (41, start(0, Kind::Reasoning)),
        (42, text(0, "想")),
        (42, start(1, read())),
        (42, text(1, "{}")),
        (42, end(1)),
        (43, start(2, Kind::Text)),
        (44, text(2, "我读")),
        (44, start(3, read())),
    ]);
    let (reply, called) = written(&session.handle(failed(5, ErrorClass::Unclassified, "HTTP 500")));
    assert_eq!(reply.len(), 2, "{reply:?}");
    assert_eq!(called.blocks, spans(&[(1000, 2000), (3000, 4000)]));
}

#[test]
fn an_interrupted_reply_times_only_what_it_keeps() {
    // 打断：收全了的调用留下，没收全的丢掉。
    let mut session = streamed(vec![
        (41, start(0, Kind::Text)),
        (42, text(0, "我读两个")),
        (42, start(1, read())),
        (43, text(1, "{}")),
        (43, end(1)),
        (44, start(2, read())),
        (44, text(2, "{")),
    ]);
    let (reply, called) = written(&session.handle(interrupt(46)));
    assert_eq!(reply.len(), 2, "{reply:?}");
    assert_eq!(called.duration_ms, Some(6000), "算到打断为止");
    assert_eq!(called.blocks, spans(&[(1000, 2000), (2000, 3000)]));
}

#[test]
fn a_delta_that_does_not_fit_is_not_counted() {
    // 收全了以后又来的字：驱动的错，这次请求按出错收，这一段不算进那一块。
    let mut session = streamed(vec![
        (41, start(0, Kind::Text)),
        (42, text(0, "好")),
        (42, end(0)),
    ]);
    let (reply, called) = written(&session.handle(delta(5, 44, text(0, "又来"))));
    assert_eq!(reply.len(), 1);
    assert_eq!(
        called.error.map(|error| error.class),
        Some(ErrorClass::BadStream)
    );
    assert_eq!(called.blocks, spans(&[(1000, 2000)]));
}

#[test]
fn a_clock_going_back_never_moves_an_end_back() {
    // 07:00:43 以后拨回 41：止还是 43。拨到发出去以前的，算 0。
    let mut session = streamed(vec![
        (42, start(0, Kind::Text)),
        (43, text(0, "好")),
        (41, text(0, "的")),
        (39, start(1, Kind::Text)),
        (39, text(1, "嗯")),
    ]);
    let (_, called) = written(&session.handle(ended(5)));
    assert_eq!(called.blocks, spans(&[(2000, 3000), (0, 0)]));
}

#[test]
fn no_reply_no_times() {
    // 发出去了，一个字都没来就出错；没发出去就出错：都没有回复，没有这一格。
    let mut session = asking();
    session.handle(sent(5));
    let (reply, called) = written(&session.handle(failed(5, ErrorClass::Unclassified, "HTTP 404")));
    assert!(reply.is_empty());
    assert_eq!((called.duration_ms, called.blocks), (Some(5000), None));
    let mut session = asking();
    let (_, called) = written(&session.handle(failed(5, ErrorClass::Auth, "no key")));
    assert_eq!(called.blocks, None);
}
