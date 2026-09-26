//! 有效历史的测试：没压缩过时全都在；压缩一次、再压一次；被动压缩保下来的尾巴；
//! 撤销去掉那一轮，连同人亲口发的触发消息；别处来的触发留着。
//! 事件都先交给账本查过，保证测的是合规的日志。

use super::*;
use crate::ledger::Ledger;

const CREATED: &str = r#"{"owner":"alice","venue":"local","policy":"sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","permission":{"level":"workspace","read_only":false}}"#;
const ALICE: &str = r#"{"kind":"person","account":"alice"}"#;
const KERNEL: &str = r#"{"kind":"kernel"}"#;
const MODEL: &str = r#"{"kind":"model","endpoint":"deepseek","model":"deepseek-v4"}"#;
const GROUP_MEMBER: &str = r#"{"kind":"external","venue":"qq:group:123456","id":"qq:10086"}"#;
const ANOTHER_SESSION: &str = r#"{"kind":"session","id":"0192f3a0-1111-7abc-8def-001122334455"}"#;
const TIMER: &str = r#"{"kind":"module","id":"timer"}"#;

/// 拼一条事件：序号、所属回合、由谁引起、种类、`body`。
fn event(seq: u64, turn: Option<u64>, by: &str, kind: &str, body: &str) -> Event {
    let turn = turn.map(|t| format!(r#""turn":{t},"#)).unwrap_or_default();
    Event::from_line(&format!(
        r#"{{"seq":{seq},"at":"2026-09-25T07:00:00.000Z","kind":"{kind}",{turn}"by":{by},"body":{body}}}"#
    ))
    .unwrap()
}

fn created() -> Event {
    event(1, None, KERNEL, "session.created", CREATED)
}

/// `by` 发来的一条消息。
fn message(seq: u64, by: &str) -> Event {
    event(seq, None, by, "message.user", r#"{"blocks":[]}"#)
}

/// 一整轮：由 `trigger` 触发，从 `start` 开始，模型回一句就结束，占三个序号。
/// 回复的请求看到了回合开始那一条为止。
fn turn(start: u64, trigger: u64) -> Vec<Event> {
    let body = format!(r#"{{"trigger":{trigger}}}"#);
    vec![
        event(start, Some(start), KERNEL, "turn.started", &body),
        event(
            start + 1,
            Some(start),
            MODEL,
            "message.assistant",
            &calls(start + 1, start, 0),
        ),
        event(
            start + 2,
            Some(start),
            KERNEL,
            "turn.ended",
            r#"{"reason":"completed"}"#,
        ),
    ]
}

/// 一条回复的 `body`：`n` 个工具调用，编号照 `seq` 编；它的请求看到了第 `seen` 条为止。
fn calls(seq: u64, seen: u64, n: u32) -> String {
    let blocks: Vec<String> = (1..=n)
        .map(|k| {
            format!(
                r#"{{"type":"tool_call","call_id":"call_{seq}_{k}","name":"read","args":"{{}}"}}"#
            )
        })
        .collect();
    format!(r#"{{"blocks":[{}],"seen":{seen}}}"#, blocks.join(","))
}

/// 回合 `turn` 里，调用 `call` 的结果。
fn result(seq: u64, turn: u64, call: &str) -> Event {
    let body = format!(r#"{{"call_id":"{call}","status":"ok","blocks":[]}}"#);
    event(seq, Some(turn), KERNEL, "tool.result", &body)
}

fn compacted(seq: u64, upto: u64) -> Event {
    let body = format!(r#"{{"upto":{upto},"summary":"…"}}"#);
    event(seq, None, ALICE, "context.compacted", &body)
}

fn reverted(seq: u64, turns: &[u64]) -> Event {
    let body = format!(r#"{{"turns":{turns:?}}}"#);
    event(seq, None, ALICE, "turn.reverted", &body)
}

/// 两个回合的会话：人问一句（2），第一轮（3 到 5）；再问一句（6），第二轮（7 到 9）。
fn two_turns() -> Vec<Event> {
    let mut events = vec![created(), message(2, ALICE)];
    events.extend(turn(3, 2));
    events.push(message(6, ALICE));
    events.extend(turn(7, 6));
    events
}

/// 把事件依次交给账本和有效历史。
fn feed(events: impl IntoIterator<Item = Event>) -> History {
    let mut ledger = Ledger::default();
    let mut history = History::default();
    for event in events {
        ledger.append(&event).unwrap();
        history.append(event);
    }
    history
}

/// 检查点之后还有效的事件，只看序号。
fn seqs(history: &History) -> Vec<u64> {
    history
        .events()
        .iter()
        .map(|event| event.seq.get())
        .collect()
}

fn checkpoint(history: &History) -> Option<u64> {
    history.checkpoint().map(|event| event.seq.get())
}

/// 照每次请求看到的范围排好以后，只看序号。
fn ordered(history: &History) -> Vec<u64> {
    history
        .ordered()
        .iter()
        .map(|event| event.seq.get())
        .collect()
}

#[test]
fn without_compaction_everything_stays() {
    let history = feed(two_turns());
    assert_eq!(seqs(&history), (1..=9).collect::<Vec<_>>());
    assert_eq!(checkpoint(&history), None);
    // 请求在路上时什么也没来，排出来就是日志的先后。
    assert_eq!(ordered(&history), seqs(&history));
}

#[test]
fn a_compaction_starts_the_history_over() {
    let mut events = two_turns();
    events.push(compacted(10, 9));
    events.push(message(11, ALICE));
    let history = feed(events);
    assert_eq!(checkpoint(&history), Some(10));
    assert_eq!(seqs(&history), vec![11]);
}

/// 被动压缩保下最近一组：替代到 5，6 到 9 原样留着，排在检查点后面。
#[test]
fn a_passive_compaction_keeps_its_tail_after_the_checkpoint() {
    let mut events = two_turns();
    events.push(compacted(10, 5));
    let history = feed(events);
    assert_eq!(checkpoint(&history), Some(10));
    assert_eq!(seqs(&history), vec![6, 7, 8, 9]);
}

/// 第二次压缩替代到 11：旧的检查点 10 和 6 到 11 都丢掉，新摘要里已经包着它们。
#[test]
fn the_latest_checkpoint_replaces_the_one_before() {
    let mut events = two_turns();
    events.push(compacted(10, 5));
    events.push(message(11, ALICE));
    events.extend(turn(12, 11));
    events.push(compacted(15, 11));
    let history = feed(events);
    assert_eq!(checkpoint(&history), Some(15));
    assert_eq!(seqs(&history), vec![12, 13, 14]);
}

/// 撤掉第二轮：7 到 9 去掉，触发它的 6 是人亲口发的，也去掉；撤销这一条本身不留。
#[test]
fn undo_takes_the_turn_and_the_message_that_asked_for_it() {
    let mut events = two_turns();
    events.push(reverted(10, &[7]));
    let history = feed(events);
    assert_eq!(seqs(&history), vec![1, 2, 3, 4, 5]);
}

#[test]
fn undo_leaves_the_other_turns_alone() {
    let mut events = two_turns();
    events.push(reverted(10, &[3]));
    let history = feed(events);
    assert_eq!(seqs(&history), vec![1, 6, 7, 8, 9]);
}

/// 群里别人说的话、另一个会话发来的消息、定时触发，都是别处来的：
/// 撤掉的只是她对它们的反应，它们自己留着。
#[test]
fn undo_keeps_triggers_that_came_from_elsewhere() {
    let mut events = vec![created(), message(2, GROUP_MEMBER)];
    events.extend(turn(3, 2));
    events.push(message(6, ANOTHER_SESSION));
    events.extend(turn(7, 6));
    events.push(event(10, None, TIMER, "ext.timer.fired", "{}"));
    events.extend(turn(11, 10));
    events.push(reverted(14, &[3, 7, 11]));
    let history = feed(events);
    assert_eq!(seqs(&history), vec![1, 2, 6, 10]);
}

/// 03 第六节那一回合，序号挪了一挪：回复 5 调了两个工具、看到 4；第二个调用的结果 6 先回来；
/// 工具还在跑时人又说了一句 7；第一个调用的结果 8；最后的回复 9 看到 8。
/// 图上的 44、48、46、47，在这里是 5、8、6、7。
#[test]
fn the_turn_from_the_drawing_is_ordered_as_drawn() {
    let events = vec![
        created(),
        message(2, ALICE),
        event(3, Some(3), KERNEL, "turn.started", r#"{"trigger":2}"#),
        event(
            4,
            Some(3),
            KERNEL,
            "context.injected",
            r#"{"kind":"env","text":"<env/>"}"#,
        ),
        event(5, Some(3), MODEL, "message.assistant", &calls(5, 4, 2)),
        result(6, 3, "call_5_2"),
        message(7, ALICE),
        result(8, 3, "call_5_1"),
        event(9, Some(3), MODEL, "message.assistant", &calls(9, 8, 0)),
        event(
            10,
            Some(3),
            KERNEL,
            "turn.ended",
            r#"{"reason":"completed"}"#,
        ),
    ];
    assert_eq!(ordered(&feed(events)), vec![1, 2, 3, 4, 5, 8, 6, 7, 9, 10]);
}

/// 请求在路上时人又说了一句（4）：回复（5）只看到 3，所以这一句排在回复后面。
#[test]
fn a_message_that_came_while_the_request_was_out_goes_after_the_reply() {
    let events = vec![
        created(),
        message(2, ALICE),
        event(3, Some(3), KERNEL, "turn.started", r#"{"trigger":2}"#),
        message(4, ALICE),
        event(5, Some(3), MODEL, "message.assistant", &calls(5, 3, 0)),
        event(
            6,
            Some(3),
            KERNEL,
            "turn.ended",
            r#"{"reason":"completed"}"#,
        ),
    ];
    assert_eq!(ordered(&feed(events)), vec![1, 2, 3, 5, 4, 6]);
}

/// 被动压缩保下最后一组：回复 9 连同它的调用和结果。尾巴里照样照请求排，
/// 工具在跑时来的那一句（10）排在结果（11）后面。
#[test]
fn the_kept_tail_after_a_compaction_is_ordered_the_same_way() {
    let mut events = vec![created(), message(2, ALICE)];
    events.extend(turn(3, 2));
    events.push(message(6, ALICE));
    events.push(event(
        7,
        Some(7),
        KERNEL,
        "turn.started",
        r#"{"trigger":6}"#,
    ));
    events.push(event(
        8,
        Some(7),
        KERNEL,
        "context.injected",
        r#"{"kind":"env","text":"<env/>"}"#,
    ));
    events.push(event(
        9,
        Some(7),
        MODEL,
        "message.assistant",
        &calls(9, 8, 1),
    ));
    events.push(message(10, ALICE));
    events.push(result(11, 7, "call_9_1"));
    events.push(compacted(12, 8));
    let history = feed(events);
    assert_eq!(checkpoint(&history), Some(12));
    assert_eq!(ordered(&history), vec![9, 11, 10]);
}
