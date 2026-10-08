//! 调用和回应照 `echo` 配对（施工 O-8，`onebot.md` 第一条「怎么走」第 4 条）：等了给的时限（出厂是 `bridge.json` 的
//! `call_timeout_seconds`）还等不到算失败；连接断了，在等的都算失败；同时在等的几个各拿各的回应。钟是停住的
//! （`start_paused`），等多久照它算，不照真的时间。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

use miyu_onebot::onebot::{CallError, Calls};

/// 测试里给的时限：故意和出厂的（`bridge.json`，`tests/tuning.rs` 守着）不一样，看得出 [`Calls`] 照给的等。
const TIMEOUT: Duration = Duration::from_secs(3);

/// 读出桥写出去的一帧动作。钟停着：一直没有的，过一分钟（停住的钟上的）就算没有，不卡住。
async fn frame(frames: &mut mpsc::Receiver<Message>) -> Value {
    let frame = tokio::time::timeout(Duration::from_secs(60), frames.recv())
        .await
        .expect("写出了一帧");
    match frame {
        Some(Message::Text(text)) => serde_json::from_str(&text).expect("是 JSON"),
        other => panic!("不是一帧文字：{other:?}"),
    }
}

#[tokio::test(start_paused = true)]
async fn a_call_without_an_answer_fails_after_the_timeout() {
    let calls = Arc::new(Calls::new(TIMEOUT));
    let (out, mut frames) = mpsc::channel(8);
    let calling = {
        let calls = Arc::clone(&calls);
        tokio::spawn(async move { calls.call(&out, "get_version_info", json!({})).await })
    };
    let sent = frame(&mut frames).await;
    assert_eq!(sent["action"], "get_version_info");
    assert!(sent["echo"].is_string(), "{sent}");
    tokio::time::sleep(TIMEOUT - Duration::from_millis(100)).await;
    assert!(!calling.is_finished(), "到时以前还在等");
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(calling.is_finished(), "过了时就不等了");
    assert_eq!(calling.await.expect("没崩"), Err(CallError::Timeout));
    // 过了时的回应不算数。
    assert!(!calls.answer(json!({"status": "ok", "echo": sent["echo"]})));
}

#[tokio::test(start_paused = true)]
async fn calls_waiting_when_the_connection_closes_fail() {
    let calls = Arc::new(Calls::new(TIMEOUT));
    let (out, mut frames) = mpsc::channel(8);
    let calling = {
        let calls = Arc::clone(&calls);
        let out = out.clone();
        tokio::spawn(async move { calls.call(&out, "send_private_msg", json!({})).await })
    };
    frame(&mut frames).await;
    calls.close();
    assert_eq!(calling.await.expect("没崩"), Err(CallError::Closed));
    assert_eq!(
        calls.call(&out, "send_private_msg", json!({})).await,
        Err(CallError::Closed),
        "关了以后再调的也算失败"
    );
}

#[tokio::test(start_paused = true)]
async fn answers_are_paired_by_echo() {
    let calls = Arc::new(Calls::new(TIMEOUT));
    let (out, mut frames) = mpsc::channel(8);
    let call = |action: &'static str| {
        let calls = Arc::clone(&calls);
        let out = out.clone();
        tokio::spawn(async move { calls.call(&out, action, json!({})).await })
    };
    let first = call("first");
    let one = frame(&mut frames).await;
    let second = call("second");
    let two = frame(&mut frames).await;
    assert_ne!(one["echo"], two["echo"], "同一条连接里不重");
    assert!(
        calls.answer(json!({"status": "ok", "retcode": 0, "data": {"n": 2}, "echo": two["echo"]}))
    );
    assert!(
        calls.answer(
            json!({"status": "failed", "retcode": 100, "message": "no", "echo": one["echo"]})
        )
    );
    assert_eq!(second.await.expect("没崩").expect("成了")["data"]["n"], 2);
    assert!(matches!(
        first.await.expect("没崩"),
        Err(CallError::Failed(_))
    ));
    assert!(
        !calls.answer(json!({"status": "ok", "echo": "nobody"})),
        "没人等的回应不认"
    );
}
