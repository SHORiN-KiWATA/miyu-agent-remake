//! 群里的命令回执撤回（施工 O-25 上，`onebot.md` 第一条「斜杠命令」第 7 条）：NapCat 回了编号，等够给的时候才撤，撤的是那个
//! 编号，经那时的连接；没回编号的、那时没连着的不撤。钟是停住的（`start_paused`），等多久照它算，不照真的时间。

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use miyu_kernel::id::VenueId;
use serde_json::{Value, json};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

use super::super::Peer;
use super::recall;
use crate::listen::bots::{Bots, Link};
use crate::onebot::{Calls, To};

/// 机器人的号。
const BOT: i64 = 30003;

/// 测试里给的：等 3 秒再撤。
const AFTER: Duration = Duration::from_secs(3);

/// 一条连着的机器人号：交回号到连接、在等的调用、写出去的帧。
fn connected() -> (
    Arc<Bots>,
    Arc<Calls>,
    mpsc::Receiver<Message>,
    mpsc::Sender<Message>,
) {
    let (out, frames) = mpsc::channel(8);
    let calls = Arc::new(Calls::new(Duration::from_secs(10)));
    let bots = Arc::new(Bots::default());
    let link = Link {
        serial: 0,
        out: out.clone(),
        calls: Arc::clone(&calls),
        peer: Arc::new(OnceLock::new()),
    };
    assert!(bots.insert(BOT, link).is_none());
    (bots, calls, frames, out)
}

/// 发到群 5 的那个场所。
fn peer() -> Peer {
    Peer {
        bot: BOT,
        to: To::Group(5),
        venue: VenueId::parse("qq:group:5").expect("合写法"),
    }
}

/// 写出去的下一帧。钟停着：一直没有的，过一分钟（停住的钟上的）就算没有，不卡住。
async fn frame(frames: &mut mpsc::Receiver<Message>) -> Value {
    let frame = tokio::time::timeout(Duration::from_secs(60), frames.recv())
        .await
        .expect("写出了一帧");
    match frame {
        Some(Message::Text(text)) => serde_json::from_str(&text).expect("是 JSON"),
        other => panic!("不是一帧文字：{other:?}"),
    }
}

/// 回一帧 `sent`：成了，`data` 照给的。
fn answer(calls: &Calls, sent: &Value, data: Value) {
    let reply = json!({"status": "ok", "retcode": 0, "data": data, "echo": sent["echo"]});
    assert!(calls.answer(reply), "有人在等");
}

#[tokio::test(start_paused = true)]
async fn the_receipt_is_recalled_only_after_the_wait() {
    let (bots, calls, mut frames, out) = connected();
    let pending = calls
        .begin(&out, "send_group_msg", json!({}))
        .await
        .expect("放得进");
    let sent = frame(&mut frames).await;
    let task = tokio::spawn(recall(pending, peer(), bots, AFTER, 6));
    answer(&calls, &sent, json!({"message_id": 77}));
    tokio::time::sleep(AFTER - Duration::from_millis(1)).await;
    assert!(frames.try_recv().is_err(), "还没到时候");
    let deleted = frame(&mut frames).await;
    assert_eq!(deleted["action"], "delete_msg");
    assert_eq!(
        deleted["params"],
        json!({"message_id": 77}),
        "撤的是回的那个编号"
    );
    answer(&calls, &deleted, Value::Null);
    task.await.expect("没崩");
}

#[tokio::test(start_paused = true)]
async fn without_an_id_or_a_connection_nothing_is_recalled() {
    let (bots, calls, mut frames, out) = connected();
    let pending = calls
        .begin(&out, "send_group_msg", json!({}))
        .await
        .expect("放得进");
    let sent = frame(&mut frames).await;
    let task = tokio::spawn(recall(pending, peer(), Arc::clone(&bots), AFTER, 6));
    answer(&calls, &sent, json!({}));
    task.await.expect("没崩");
    let pending = calls
        .begin(&out, "send_group_msg", json!({}))
        .await
        .expect("放得进");
    let sent = frame(&mut frames).await;
    let task = tokio::spawn(recall(pending, peer(), Arc::clone(&bots), AFTER, 6));
    answer(&calls, &sent, json!({"message_id": 78}));
    bots.remove(BOT, 0);
    task.await.expect("没崩");
    assert!(frames.try_recv().is_err(), "没回编号的、那时没连着的都不撤");
}
