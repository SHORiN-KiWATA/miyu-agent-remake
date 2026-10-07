//! 回合索引的一条（`memory.md` 第一条第 1 到 5 款）。

use miyu_kernel::event::Event;
use miyu_kernel::id::{Seq, SessionId, TurnId};
use miyu_kernel::time::Timestamp;

use super::*;

const ALICE: &str = r#"{"kind":"person","account":"admin"}"#;
const MODEL: &str = r#"{"kind":"model","endpoint":"deepseek","model":"deepseek-v4"}"#;
const KERNEL: &str = r#"{"kind":"kernel"}"#;

fn at(seq: u64) -> String {
    format!("2026-10-07T08:00:{:02}.000Z", seq % 60)
}

fn line(seq: u64, kind: &str, turn: Option<u64>, by: &str, body: &str) -> Event {
    let turn = turn.map_or(String::new(), |turn| format!(r#","turn":{turn}"#));
    Event::from_line(&format!(
        r#"{{"seq":{seq},"at":"{}","kind":"{kind}"{turn},"by":{by},"body":{body}}}"#,
        at(seq)
    ))
    .unwrap()
}

fn said(seq: u64, by: &str, text: &str) -> Event {
    line(
        seq,
        "message.user",
        None,
        by,
        &format!(r#"{{"blocks":[{{"type":"text","text":{}}}]}}"#, json(text)),
    )
}

fn started(seq: u64, trigger: Option<u64>) -> Event {
    let body = trigger.map_or("{}".to_string(), |t| format!(r#"{{"trigger":{t}}}"#));
    line(seq, "turn.started", Some(seq), KERNEL, &body)
}

fn replied(seq: u64, turn: u64, text: &str) -> Event {
    line(
        seq,
        "message.assistant",
        Some(turn),
        MODEL,
        &format!(
            r#"{{"blocks":[{{"type":"text","text":{}}}],"seen":{}}}"#,
            json(text),
            seq - 1
        ),
    )
}

fn ended(seq: u64, turn: u64, reason: &str) -> Event {
    line(
        seq,
        "turn.ended",
        Some(turn),
        KERNEL,
        &format!(r#"{{"reason":"{reason}"}}"#),
    )
}

fn reverted(seq: u64, turns: &[u64]) -> Event {
    line(
        seq,
        "turn.reverted",
        None,
        ALICE,
        &format!(r#"{{"turns":{turns:?}}}"#),
    )
}

fn unreverted(seq: u64, turns: &[u64]) -> Event {
    line(
        seq,
        "turn.unreverted",
        None,
        ALICE,
        &format!(r#"{{"turns":{turns:?}}}"#),
    )
}

fn json(text: &str) -> String {
    serde_json::to_string(text).unwrap()
}

fn turn(seq: u64) -> TurnId {
    TurnId::new(Seq::new(seq).unwrap())
}

fn item(turn_seq: u64, text: &str) -> TurnItem {
    TurnItem {
        turn: turn(turn_seq),
        text: text.to_string(),
        at: Timestamp::parse(&at(turn_seq)).unwrap(),
    }
}

fn feed(events: &[Event]) -> Vec<Change> {
    let mut feed = TurnFeed::default();
    events.iter().flat_map(|event| feed.see(event)).collect()
}

/// 一轮：人说一句，她答两次（中间一次调工具），结束。
fn one_turn(first: u64, by: &str, words: &str, answer: &str) -> Vec<Event> {
    vec![
        said(first, by, words),
        started(first + 1, Some(first)),
        replied(first + 2, first + 1, "我先看看。"),
        replied(first + 3, first + 1, answer),
        ended(first + 4, first + 1, "completed"),
    ]
}

#[test]
fn a_turn_opened_by_a_person_is_put_when_it_ends() {
    assert_eq!(
        feed(&one_turn(2, ALICE, " 我用 N 卡 ", "好，记住了。")),
        [Change::Put(item(3, "我用 N 卡\n\n好，记住了。"))]
    );
}

#[test]
fn someone_from_a_platform_counts_as_a_person() {
    let by = r#"{"kind":"external","venue":"qq:group:1","id":"qq:10086"}"#;
    assert_eq!(
        feed(&one_turn(2, by, "早", "早上好")),
        [Change::Put(item(3, "早\n\n早上好"))]
    );
}

#[test]
fn turns_not_opened_by_a_person_are_not_put() {
    for by in [
        r#"{"kind":"harness","name":"claude-code"}"#,
        r#"{"kind":"session","id":"0192f3a0-2222-7abc-8def-5566778899aa"}"#,
        KERNEL,
    ] {
        assert_eq!(feed(&one_turn(2, by, "跑一下测试", "跑过了")), [], "{by}");
    }
    // 没有触发的（手动压缩、清空单开的那一轮）。
    let events = [
        started(2, None),
        replied(3, 2, "压好了"),
        ended(4, 2, "completed"),
    ];
    assert_eq!(feed(&events), []);
}

#[test]
fn an_interrupted_turn_is_put_and_an_empty_one_is_not() {
    let events = [
        said(2, ALICE, "讲个故事"),
        started(3, Some(2)),
        replied(4, 3, "从前有座山"),
        ended(5, 3, "interrupted"),
    ];
    assert_eq!(
        feed(&events),
        [Change::Put(item(3, "讲个故事\n\n从前有座山"))]
    );
    let events = [
        said(2, ALICE, ""),
        started(3, Some(2)),
        ended(4, 3, "interrupted"),
    ];
    assert_eq!(feed(&events), []);
    let events = [
        said(2, ALICE, "在吗"),
        started(3, Some(2)),
        ended(4, 3, "error"),
    ];
    assert_eq!(feed(&events), [Change::Put(item(3, "在吗"))]);
}

#[test]
fn the_last_reply_with_words_is_the_answer() {
    let events = [
        said(2, ALICE, "看看 src"),
        started(3, Some(2)),
        replied(4, 3, "src 下有两个文件。"),
        replied(5, 3, "   "),
        ended(6, 3, "completed"),
    ];
    assert_eq!(
        feed(&events),
        [Change::Put(item(3, "看看 src\n\nsrc 下有两个文件。"))]
    );
}

#[test]
fn the_trigger_decides_even_after_queued_messages() {
    // 一句排着的、一句触发的：触发的那句是这一轮的话。
    let events = [
        said(2, ALICE, "先别管"),
        said(3, ALICE, "看看 tests"),
        started(4, Some(3)),
        replied(5, 4, "好"),
        ended(6, 4, "completed"),
    ];
    assert_eq!(feed(&events), [Change::Put(item(4, "看看 tests\n\n好"))]);
}

#[test]
fn reverting_removes_and_unreverting_asks_for_the_log() {
    let mut events = one_turn(2, ALICE, "第一句", "第一答");
    events.extend(one_turn(7, ALICE, "第二句", "第二答"));
    events.push(reverted(12, &[3, 8]));
    events.push(unreverted(13, &[3, 8]));
    let changes = feed(&events);
    assert_eq!(
        changes[2..],
        [
            Change::Remove(turn(3)),
            Change::Remove(turn(8)),
            Change::Restored(vec![turn(3), turn(8)]),
            Change::Lost(vec![turn(3), turn(8)])
        ]
    );
}

#[test]
fn replay_keeps_what_is_left_at_the_end() {
    let mut events = one_turn(2, ALICE, "第一句", "第一答");
    events.extend(one_turn(7, ALICE, "第二句", "第二答"));
    events.push(reverted(12, &[8]));
    assert_eq!(replay(&events), [item(3, "第一句\n\n第一答")]);
    events.push(unreverted(13, &[8]));
    assert_eq!(
        replay(&events),
        [item(3, "第一句\n\n第一答"), item(8, "第二句\n\n第二答")]
    );
}

#[test]
fn the_key_is_the_session_and_the_turn() {
    let session = SessionId::parse("0192f3a0-1111-7abc-8def-001122334455").unwrap();
    assert_eq!(
        key(&session, turn(42)),
        "0192f3a0-1111-7abc-8def-001122334455/42"
    );
}

#[test]
fn priming_restores_the_open_turn_and_gives_only_what_comes_after() {
    let mut events = one_turn(2, ALICE, "第一句", "第一答");
    events.push(said(7, ALICE, "第二句"));
    events.push(started(8, Some(7)));
    // 照到第 7 条：第一轮已经在库里了，第二轮还在进行。
    let (mut feed, changes) = TurnFeed::primed(&events, Some(Seq::new(7).unwrap()));
    assert_eq!(changes, []);
    let rest = [replied(9, 8, "第二答"), ended(10, 8, "completed")];
    let changes: Vec<Change> = rest.iter().flat_map(|event| feed.see(event)).collect();
    assert_eq!(changes, [Change::Put(item(8, "第二句\n\n第二答"))]);
    // 一条都没照过的，交回全部；铺完以后恢复照旧交回要读回。
    let (mut feed, changes) = TurnFeed::primed(&events, None);
    assert_eq!(changes, [Change::Put(item(3, "第一句\n\n第一答"))]);
    assert_eq!(
        feed.see(&unreverted(11, &[3])),
        [Change::Restored(vec![turn(3)]), Change::Lost(vec![turn(3)])]
    );
}

#[test]
fn priming_up_to_the_end_of_a_turn_gives_nothing_again() {
    // 照到的正好是那一轮的 `turn.ended`：那一条已经在库里了，不再交。
    let events = one_turn(2, ALICE, "第一句", "第一答");
    let (_, changes) = TurnFeed::primed(&events, Some(Seq::new(6).unwrap()));
    assert_eq!(changes, []);
    let (_, changes) = TurnFeed::primed(&events, Some(Seq::new(5).unwrap()));
    assert_eq!(changes, [Change::Put(item(3, "第一句\n\n第一答"))]);
}

#[test]
fn queued_words_are_dropped_once_a_turn_starts() {
    // 排着的几句只有最后一句触发，前面的不会再触发哪一轮：丢掉，长会话不在内存里越攒越多。
    let mut feed = TurnFeed::default();
    for event in [
        said(2, ALICE, "一"),
        said(3, ALICE, "二"),
        started(4, Some(3)),
    ] {
        feed.see(&event);
    }
    assert!(feed.said.is_empty(), "{:?}", feed.said);
    feed.see(&said(5, ALICE, "三"));
    assert_eq!(feed.said.len(), 1);
}

#[test]
fn priming_through_an_undo_and_restore_gives_the_restore_and_the_item() {
    // 不是人开的那一轮（别的 harness）撤销了也要埋、恢复了也要揭：交 `Restored`，没有字可放。
    let mut events = one_turn(
        2,
        r#"{"kind":"harness","name":"claude-code"}"#,
        "跑测试",
        "跑过了",
    );
    events.extend(one_turn(7, ALICE, "第二句", "第二答"));
    events.push(reverted(12, &[3, 8]));
    events.push(unreverted(13, &[3, 8]));
    let (_, changes) = TurnFeed::primed(&events, Some(Seq::new(11).unwrap()));
    assert_eq!(
        changes,
        [
            Change::Remove(turn(3)),
            Change::Remove(turn(8)),
            Change::Restored(vec![turn(3), turn(8)]),
            Change::Put(item(8, "第二句\n\n第二答"))
        ]
    );
}
