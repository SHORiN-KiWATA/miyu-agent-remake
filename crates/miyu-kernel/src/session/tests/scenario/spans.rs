//! 场景：回复每一块的起止（施工 2-3 补，`kernel/session.md`「收回复」）。替身每送进一条输入，时钟往后走一秒；一块
//! 的增量照开始、全文、收全送（说了一半断的不收全），所以一块从开始那一秒到全文那一秒，收全的那一秒不算。

use super::*;
use crate::event::BlockSpan;

/// 一块一项，从发出去算起的毫秒数：起、止。
fn spans(pairs: &[(u64, u64)]) -> Option<Vec<BlockSpan>> {
    Some(
        pairs
            .iter()
            .map(|&(start_ms, end_ms)| BlockSpan { start_ms, end_ms })
            .collect(),
    )
}

#[test]
fn a_thought_a_word_and_a_call_each_get_their_times() {
    let mut stage = stage();
    stage.model([
        Line::calls("我看看。", &[("read", r#"{"path":"a"}"#)]).thinking("先读 a。"),
        Line::says("看完了。"),
    ]);
    stage.tools([Play::done("A")]);
    stage.say("看看 a");
    let calls = stage.model_calls();
    // 发出去以后：思考第 1、2 秒（第 3 秒收全），正文第 4、5 秒，调用第 7、8 秒，第 10 秒说完。
    assert_eq!(calls[0].first_token_ms, Some(1000));
    assert_eq!(calls[0].duration_ms, Some(10_000));
    assert_eq!(
        calls[0].blocks,
        spans(&[(1000, 2000), (4000, 5000), (7000, 8000)])
    );
    assert_eq!(calls[1].blocks, spans(&[(1000, 2000)]));
}

#[test]
fn a_reply_broken_halfway_times_only_the_blocks_it_keeps() {
    // 想了、说了、调了，都没收全就断了：调用丢掉，只记思考和正文。不再来的错，这一轮就此结束。
    let mut stage = stage();
    stage.model([Line {
        calls: vec![("read".to_string(), "{}".to_string())],
        ..Line::breaks("我读", ErrorClass::Unclassified, "HTTP 500").thinking("读 a")
    }]);
    stage.say("读 a");
    assert_eq!(
        story(&stage),
        opening_then(&[
            "6 message.assistant model t3",
            "7 model.called:error kernel t3",
            "8 turn.ended:error kernel t3",
        ])
    );
    let Body::MessageAssistant(half) = &stage.log()[5].body else {
        panic!("6 号是半截回复");
    };
    assert_eq!(half.blocks.len(), 2, "{:?}", half.blocks);
    assert_eq!(
        stage.model_calls()[0].blocks,
        spans(&[(1000, 2000), (3000, 4000)])
    );
}

#[test]
fn a_failure_before_any_word_has_no_times() {
    let mut stage = stage();
    stage.model([Line::fails(ErrorClass::Unclassified, "HTTP 404")]);
    stage.say("hi");
    let calls = stage.model_calls();
    assert_eq!(calls[0].duration_ms, Some(1000));
    assert_eq!(calls[0].blocks, None);
}

#[test]
fn an_interrupted_reply_times_what_it_said() {
    let mut stage = stage();
    stage.model([Line::says("说到一半").thinking("想").held()]);
    stage.say("hi");
    stage.interrupt(Queued::Send);
    assert_eq!(
        stage.model_calls()[0].blocks,
        spans(&[(1000, 2000), (4000, 5000)])
    );
}
