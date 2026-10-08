//! 怎么切一页（施工 9-6 下）：边界落在回合之间；数够轮数、到了字节的上限停，至少一整轮；切点前的触发消息带上；数到第一轮
//! 连前面的一起给。每条事件当 10 字节算。

use super::*;

/// 一条事件：`kind` 照这几种拼，序号是 `seq`，回合是 `turn`。
fn event(seq: u64, kind: &str, turn: u64) -> Event {
    let line = match kind {
        "said" => format!(
            r#"{{"seq":{seq},"at":"2026-10-08T01:00:00.000Z","kind":"message.user","by":{{"kind":"person","account":"alice"}},"body":{{"blocks":[{{"type":"text","text":"hi"}}]}}}}"#
        ),
        "start" => format!(
            r#"{{"seq":{seq},"at":"2026-10-08T01:00:00.000Z","kind":"turn.started","turn":{seq},"by":{{"kind":"kernel"}},"body":{{"trigger":{}}}}}"#,
            seq - 1
        ),
        "end" => format!(
            r#"{{"seq":{seq},"at":"2026-10-08T01:00:00.000Z","kind":"turn.ended","turn":{turn},"by":{{"kind":"kernel"}},"body":{{"reason":"completed"}}}}"#
        ),
        _ => format!(
            r#"{{"seq":{seq},"at":"2026-10-08T01:00:00.000Z","kind":"session.meta_changed","by":{{"kind":"person","account":"alice"}},"body":{{"title":"t"}}}}"#
        ),
    };
    Event::from_line(&line).expect("合写法")
}

/// 一段日志：第 1 条改了标题（当成开头的那几条），之后三轮，每一轮是人说的话、开回合、结束；第二轮结束后又改了一次标题。
///
/// ```text
/// 1 标题 | 2 话 3 开 4 完 | 5 话 6 开 7 完 8 标题 | 9 话 10 开 11 完
/// ```
fn log() -> Vec<Event> {
    vec![
        event(1, "meta", 0),
        event(2, "said", 0),
        event(3, "start", 0),
        event(4, "end", 3),
        event(5, "said", 0),
        event(6, "start", 0),
        event(7, "end", 6),
        event(8, "meta", 0),
        event(9, "said", 0),
        event(10, "start", 0),
        event(11, "end", 10),
    ]
}

fn seqs(page: &Page<'_>) -> Vec<u64> {
    page.events.iter().map(|event| event.seq.get()).collect()
}

fn ten(_: &Event) -> usize {
    10
}

#[test]
fn the_newest_page_is_whole_turns_with_their_triggers() {
    let events = log();
    let newest = page(&events, None, 2, 1 << 20, ten);
    assert_eq!(
        seqs(&newest),
        [5, 6, 7, 8, 9, 10, 11],
        "第二、三轮，第二轮的话在切点前、带上"
    );
    assert_eq!((newest.first, newest.last), (Some(6), Some(11)));
    assert!(newest.more);
    assert!(!newest.capped);
    let older = page(&events, newest.first, 2, 1 << 20, ten);
    assert_eq!(
        seqs(&older),
        [1, 2, 3, 4, 5],
        "数到第一轮，连前面的一起给；第 5 条和上一页重了"
    );
    assert_eq!((older.first, older.last), (Some(1), Some(5)));
    assert!(!older.more);
}

#[test]
fn the_byte_cap_stops_between_turns_but_gives_at_least_one() {
    let events = log();
    let capped = page(&events, None, 20, 35, ten);
    assert_eq!(
        seqs(&capped),
        [9, 10, 11],
        "最新一轮 20 字节（它的话在切点前、不算），再加一轮就超了"
    );
    assert_eq!(capped.first, Some(10));
    assert!(capped.more && capped.capped);
    let tiny = page(&events, None, 20, 5, ten);
    assert_eq!(seqs(&tiny), [9, 10, 11], "一轮自己超了也整轮给");
    assert!(tiny.capped);
}

#[test]
fn what_comes_before_the_first_turn_waits_for_the_next_page_when_it_does_not_fit() {
    let events = log();
    // 第一轮 2 到 5 之前只有第 1 条；从第 6 条往前：第一轮 3、4、5（30 字节），加上第 1、2 条（20）就超了 45。
    let first_turn = page(&events, Some(6), 20, 45, ten);
    assert_eq!(seqs(&first_turn), [2, 3, 4, 5]);
    assert_eq!(first_turn.first, Some(3));
    assert!(first_turn.more && first_turn.capped);
    let rest = page(&events, first_turn.first, 20, 45, ten);
    assert_eq!(seqs(&rest), [1, 2], "没有回合的：整段给");
    assert!(!rest.more && !rest.capped);
}

#[test]
fn a_log_without_turns_and_an_empty_range() {
    let events = log();
    let opening = page(&events[..2], None, 20, 1 << 20, ten);
    assert_eq!(seqs(&opening), [1, 2]);
    assert!(!opening.more);
    let empty = page(&events, Some(1), 20, 1 << 20, ten);
    assert_eq!(
        (seqs(&empty), empty.first, empty.last, empty.more),
        (Vec::new(), None, None, false)
    );
}
