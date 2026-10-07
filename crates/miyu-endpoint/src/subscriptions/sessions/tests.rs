//! 转发任务（施工 9-5）：回应先写；号不大于回应里那份列表的推送丢掉，之后的照先后写；掉了队推 `resync` 停下。

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{broadcast, mpsc};

use super::SessionsForwarder;
use crate::listing::Change;

fn change(number: u64) -> Arc<Change> {
    Arc::new(Change {
        number,
        line: format!("push {number}"),
    })
}

async fn lines(out: &mut mpsc::Receiver<String>, n: usize) -> Vec<String> {
    let mut got = Vec::new();
    for _ in 0..n {
        let line = tokio::time::timeout(Duration::from_secs(5), out.recv())
            .await
            .expect("五秒内写了")
            .expect("写队列开着");
        got.push(line);
    }
    got
}

#[tokio::test]
async fn the_reply_goes_first_and_older_pushes_are_dropped() {
    let (pushes, receiver) = broadcast::channel(16);
    let (tx, mut out) = mpsc::channel(16);
    let mut forwarder = SessionsForwarder::start(receiver, 2, tx);
    // 回应之前就到了的推送：号不大于 2 的已经在列表里了。
    for number in 1..=4 {
        drop(pushes.send(change(number)));
    }
    forwarder.reply("reply".to_string()).expect("头一次交");
    assert_eq!(lines(&mut out, 3).await, ["reply", "push 3", "push 4"]);
    assert_eq!(
        forwarder.reply("again".to_string()),
        Err("again".to_string()),
        "只交一次"
    );
}

#[tokio::test]
async fn falling_behind_sends_resync_and_stops() {
    let (pushes, receiver) = broadcast::channel(1);
    let (tx, mut out) = mpsc::channel(16);
    let mut forwarder = SessionsForwarder::start(receiver, 0, tx);
    for number in 1..=3 {
        drop(pushes.send(change(number)));
    }
    forwarder.reply("reply".to_string()).expect("头一次交");
    let got = lines(&mut out, 2).await;
    assert_eq!(
        got[1],
        r#"{"jsonrpc":"2.0","method":"resync","params":{"stream":"sessions"}}"#
    );
    drop(pushes.send(change(4)));
    let next = tokio::time::timeout(Duration::from_millis(200), out.recv()).await;
    assert!(matches!(next, Ok(None) | Err(_)), "停了：{next:?}");
}
