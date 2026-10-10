//! 贴表情（施工 O-25 下，`onebot.md` 第一条「贴表情」）：贴着的几条照推来的 `turn.started`、`turn.joined` 记下进了哪一轮，那一轮
//! 的 `venue.delivered`、`turn.ended` 发摘的信号，别的轮、别的会话、没进哪一轮的不发，同一条只发一次；贴、等、摘的任务：信号来了
//! 或者到了时候摘一次，经那时的连接，贴不上的不摘，信号的一头放下了（桥在停）不摘。钟是停住的（`start_paused`）。

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use miyu_kernel::event::Event;
use miyu_kernel::id::VenueId;
use miyu_kernel::time::Timestamp;
use serde_json::{Value, json};
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::tungstenite::Message;

use super::super::Peer;
use super::{Mark, Reactions, mark};
use crate::listen::bots::{Bots, Link};
use crate::onebot::{Calls, To};

/// 两个群会话的编号。
const ONE: &str = "01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91";
const TWO: &str = "01a0d78c-ca52-7d19-8b64-0e3f5a7c2d92";

/// 机器人的号。
const BOT: i64 = 30003;

/// 测试里给的：贴了以后十分钟摘。
const AFTER: Duration = Duration::from_secs(600);

/// 照日志的写法读一条事件：序号 `seq`、种类 `kind`、回合编号 `turn`、`body` 照给的。
fn event(seq: u64, kind: &str, turn: Option<u64>, body: Value) -> Event {
    let at = Timestamp::from_unix_millis(1_760_000_000_000).expect("在范围里");
    let mut line =
        json!({"seq": seq, "at": at, "kind": kind, "by": {"kind": "kernel"}, "body": body});
    if let Some(turn) = turn {
        line["turn"] = json!(turn);
    }
    serde_json::from_value(line).expect("读得成事件")
}

/// 第 `seq` 条开的一轮，由那几条触发。
fn started(seq: u64, triggers: &[u64]) -> Event {
    let body = json!({"trigger": triggers.last(), "triggers": triggers});
    event(seq, "turn.started", Some(seq), body)
}

/// 并进第 `turn` 轮的那几条。
fn joined(seq: u64, turn: u64, triggers: &[u64]) -> Event {
    event(
        seq,
        "turn.joined",
        Some(turn),
        json!({"triggers": triggers}),
    )
}

/// 第 `turn` 轮发出去的一段。
fn delivered(seq: u64, turn: u64) -> Event {
    let body = json!({"line": ONE, "turn": turn, "to": [], "msg": "90001", "text": "在。"});
    let mut event = event(seq, "venue.delivered", None, body);
    event.by = serde_json::from_value(json!({"kind": "module", "id": "onebot"})).expect("合写法");
    event
}

/// 第 `turn` 轮完了。
fn ended(seq: u64, turn: u64) -> Event {
    event(
        seq,
        "turn.ended",
        Some(turn),
        json!({"reason": "completed"}),
    )
}

/// 摘的信号发了没有：发了是真，还没发是假；那一头放下了的照实说。
fn signalled(off: &mut oneshot::Receiver<()>) -> Option<bool> {
    match off.try_recv() {
        Ok(()) => Some(true),
        Err(oneshot::error::TryRecvError::Empty) => Some(false),
        Err(oneshot::error::TryRecvError::Closed) => None,
    }
}

#[test]
fn the_first_piece_of_the_turn_that_took_it_signals() {
    let mut reactions = Reactions::new("289".to_string(), AFTER);
    let mut off = reactions.hold(ONE, 5);
    reactions.seen(ONE, &started(10, &[4, 5]));
    assert_eq!(signalled(&mut off), Some(false), "进了一轮还没说");
    reactions.seen(ONE, &delivered(11, 9));
    assert_eq!(signalled(&mut off), Some(false), "别的轮的不算");
    // 别的会话的序号、回合编号各数各的：它那边同一个序号贴着、还没进哪一轮，也不算这边的。
    let mut there = reactions.hold(TWO, 5);
    reactions.seen(TWO, &delivered(11, 10));
    assert_eq!(signalled(&mut off), Some(false), "别的会话的不算");
    assert_eq!(signalled(&mut there), Some(false), "没进哪一轮的不算");
    reactions.seen(ONE, &delivered(12, 10));
    assert_eq!(signalled(&mut off), Some(true));
    // 发过了就拿掉：再来一段、这一轮完了不再发。
    reactions.seen(ONE, &delivered(13, 10));
    reactions.seen(ONE, &ended(14, 10));
    assert_eq!(signalled(&mut off), None, "只发一次");
}

#[test]
fn a_joined_message_goes_with_that_turn_and_its_end_signals() {
    let mut reactions = Reactions::new("289".to_string(), AFTER);
    let mut off = reactions.hold(ONE, 7);
    reactions.seen(ONE, &started(10, &[5]));
    reactions.seen(ONE, &joined(12, 10, &[7]));
    reactions.seen(ONE, &ended(13, 9));
    assert_eq!(signalled(&mut off), Some(false), "别的轮完了不算");
    reactions.seen(ONE, &ended(14, 10));
    assert_eq!(signalled(&mut off), Some(true));
}

#[test]
fn a_message_no_turn_took_is_not_signalled() {
    let mut reactions = Reactions::new("289".to_string(), AFTER);
    let mut off = reactions.hold(ONE, 8);
    reactions.seen(TWO, &started(10, &[8]));
    reactions.seen(ONE, &started(11, &[6]));
    reactions.seen(ONE, &delivered(12, 11));
    reactions.seen(ONE, &ended(13, 11));
    reactions.seen(TWO, &ended(14, 10));
    assert_eq!(signalled(&mut off), Some(false));
    // 进了一轮以后，后来别的一轮又写着它（照说不会）：照先进的那一轮。
    reactions.seen(ONE, &started(15, &[8]));
    reactions.seen(ONE, &joined(16, 20, &[8]));
    reactions.seen(ONE, &ended(17, 20));
    assert_eq!(signalled(&mut off), Some(false));
    reactions.seen(ONE, &ended(18, 15));
    assert_eq!(signalled(&mut off), Some(true));
}

#[test]
fn taking_one_off_signals_it_once() {
    let mut reactions = Reactions::new("289".to_string(), AFTER);
    let mut off = reactions.hold(ONE, 9);
    let mut other = reactions.hold(ONE, 10);
    reactions.off(ONE, 9);
    assert_eq!(signalled(&mut off), Some(true));
    reactions.off(ONE, 9);
    reactions.seen(ONE, &started(11, &[9, 10]));
    reactions.seen(ONE, &ended(12, 11));
    assert_eq!(signalled(&mut off), None, "拿掉了不再发");
    assert_eq!(signalled(&mut other), Some(true), "别的照旧");
}

/// 一条连着的机器人号：交回号到连接、在等的调用、写出去的帧。
fn connected() -> (Arc<Bots>, Arc<Calls>, mpsc::Receiver<Message>) {
    let (out, frames) = mpsc::channel(8);
    let calls = Arc::new(Calls::new(Duration::from_secs(10)));
    let bots = Arc::new(Bots::default());
    let link = Link {
        serial: 0,
        out,
        calls: Arc::clone(&calls),
        peer: Arc::new(OnceLock::new()),
    };
    assert!(bots.insert(BOT, link).is_none());
    (bots, calls, frames)
}

/// 在群 5 里平台编号是 77 的那一条上贴 289。
fn target() -> Mark {
    Mark {
        peer: Peer {
            bot: BOT,
            to: To::Group(5),
            venue: VenueId::parse("qq:group:5").expect("合写法"),
        },
        message: "77".to_string(),
        emoji: "289".to_string(),
    }
}

/// 写出去的下一帧。钟停着：一直没有的，过一小时（停住的钟上的）就算没有，不卡住。
async fn frame(frames: &mut mpsc::Receiver<Message>) -> Value {
    let frame = tokio::time::timeout(Duration::from_secs(3600), frames.recv())
        .await
        .expect("写出了一帧");
    match frame {
        Some(Message::Text(text)) => serde_json::from_str(&text).expect("是 JSON"),
        other => panic!("不是一帧文字：{other:?}"),
    }
}

/// 回一帧 `sent`：成了的照 `ok`，不成的回失败。
fn answer(calls: &Calls, sent: &Value, ok: bool) {
    let status = if ok { "ok" } else { "failed" };
    let reply = json!({"status": status, "retcode": 0, "data": null, "echo": sent["echo"]});
    assert!(calls.answer(reply), "有人在等");
}

/// 一帧是 `set_msg_emoji_like`，`set` 照给的。
fn assert_reaction(frame: &Value, set: bool) {
    assert_eq!(frame["action"], "set_msg_emoji_like", "{frame}");
    assert_eq!(
        frame["params"],
        json!({"message_id": "77", "emoji_id": "289", "set": set})
    );
}

#[tokio::test(start_paused = true)]
async fn the_signal_takes_it_off_once() {
    let (bots, calls, mut frames) = connected();
    let (signal, off) = oneshot::channel();
    let task = tokio::spawn(mark(target(), off, bots, AFTER));
    let put = frame(&mut frames).await;
    assert_reaction(&put, true);
    answer(&calls, &put, true);
    tokio::time::sleep(Duration::from_secs(1)).await;
    assert!(frames.try_recv().is_err(), "没有信号、没到时候不摘");
    signal.send(()).expect("还在等");
    let taken = frame(&mut frames).await;
    assert_reaction(&taken, false);
    answer(&calls, &taken, true);
    task.await.expect("没崩");
    assert!(frames.try_recv().is_err(), "只摘一次");
}

#[tokio::test(start_paused = true)]
async fn without_a_signal_it_comes_off_in_time() {
    let (bots, calls, mut frames) = connected();
    let (_signal, off) = oneshot::channel();
    let task = tokio::spawn(mark(target(), off, bots, AFTER));
    let put = frame(&mut frames).await;
    answer(&calls, &put, true);
    tokio::time::sleep(AFTER - Duration::from_millis(1)).await;
    assert!(frames.try_recv().is_err(), "还没到时候");
    let taken = frame(&mut frames).await;
    assert_reaction(&taken, false);
    answer(&calls, &taken, false);
    task.await.expect("没崩");
}

#[tokio::test(start_paused = true)]
async fn what_was_not_put_on_is_not_taken_off() {
    let (bots, calls, mut frames) = connected();
    let (signal, off) = oneshot::channel();
    let task = tokio::spawn(mark(target(), off, bots, AFTER));
    let put = frame(&mut frames).await;
    answer(&calls, &put, false);
    task.await.expect("没崩");
    assert!(signal.send(()).is_err(), "任务已经完了");
    assert!(frames.try_recv().is_err(), "贴不上的不摘");
}

#[tokio::test(start_paused = true)]
async fn a_dropped_signal_leaves_it_on() {
    let (bots, calls, mut frames) = connected();
    let (signal, off) = oneshot::channel::<()>();
    let task = tokio::spawn(mark(target(), off, bots, AFTER));
    let put = frame(&mut frames).await;
    answer(&calls, &put, true);
    drop(signal);
    task.await.expect("没崩");
    assert!(frames.try_recv().is_err(), "桥在停：不摘");
}

#[tokio::test(start_paused = true)]
async fn the_connection_of_that_moment_is_used() {
    let (bots, calls, mut frames) = connected();
    let (signal, off) = oneshot::channel();
    let task = tokio::spawn(mark(target(), off, Arc::clone(&bots), AFTER));
    let put = frame(&mut frames).await;
    answer(&calls, &put, true);
    tokio::time::sleep(Duration::from_secs(1)).await;
    bots.remove(BOT, 0);
    signal.send(()).expect("还在等");
    task.await.expect("没崩");
    assert!(frames.try_recv().is_err(), "那时没连着的摘不了");
    // 一开始就没连着的也不贴。
    let (_signal, off) = oneshot::channel();
    mark(target(), off, bots, AFTER).await;
    assert!(frames.try_recv().is_err());
}
