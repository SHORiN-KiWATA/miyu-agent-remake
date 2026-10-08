//! 订阅时补发（施工 3-8 六补）：掉了队的头带上最后看到的序号重新订阅，掉的那一截补得回来；补的同时会话一直在追加，
//! 补的和之后推的合起来就是日志，不丢不重（真核心走一遍）。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use crate::support::{Client, Home, events, logged};
use miyu_endpoint::Core;
use miyu_session::testkit::{Play, Script};

#[tokio::test]
async fn a_lagged_head_catches_up_with_after() {
    let home = Home::new();
    // 几千段增量：远多于写队列（256 行）、会话的推送队列（1024 份），这个订阅一定掉队（和 `subscribe.rs` 一样）。
    let script = Script::new([Play::Floods(5000), Play::Says("好。")]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.subscribe("c2", &session).await;
    client
        .line(&json!({"jsonrpc": "2.0", "id": "c3", "method": "session.send", "params": {"session": session, "text": "hi"}}).to_string())
        .await;
    home.until_turns(&session, 1).await;
    let mut seen = Vec::new();
    let mut resynced = false;
    while let Some(next) = client.next_within(Duration::from_millis(500)).await {
        if next["method"] == json!("resync") {
            resynced = true;
        } else if next["method"] == json!("event") && next["params"]["event"].get("seq").is_some() {
            seen.push(next["params"]["event"].clone());
        }
    }
    assert!(resynced, "掉了队");
    let log = logged(&home, &session);
    assert!(
        seen.len() < log.len(),
        "掉了一截：{} / {}",
        seen.len(),
        log.len()
    );

    // 带上最后看到的序号重新订阅：补回掉的那一截。
    let last = seen
        .last()
        .map_or(0, |event| event["seq"].as_u64().expect("整数"));
    let (pushed, reply) = client.subscribe_after("c4", &session, json!(last)).await;
    assert_eq!(reply["result"]["upto"], json!(log.len()), "{reply}");
    seen.extend(events(&pushed));
    // 造完会话才订阅：第一条 `session.created` 本来就没推过来。
    assert_eq!(seen, log[1..], "看到的和补的合起来就是日志");
}

/// 一个头从头订阅，读到会话说完 `turns` 轮为止：交回推过来的、落了盘的事件（瞬时的不算），和回应。
async fn follow(core: Arc<Core>, session: String, turns: usize) -> (Vec<Value>, Value) {
    let mut client = Client::connect(core);
    client.hello().await;
    let (pushed, reply) = client.subscribe_after("r1", &session, json!(0)).await;
    let mut seen = events(&pushed);
    let ended = |seen: &[Value]| {
        seen.iter()
            .filter(|event| event["kind"] == json!("turn.ended"))
            .count()
    };
    while ended(&seen) < turns {
        let next = client.next().await.expect("没断开");
        assert_ne!(next["method"], json!("resync"), "不该掉队");
        seen.extend(events(&[next]));
    }
    let persisted = seen
        .into_iter()
        .filter(|event| event.get("seq").is_some())
        .collect();
    (persisted, reply)
}

/// 另一个头一句接一句地说，会话一直在追加；中途几个头先后从头订阅：每个头补的、之后推的合起来都和日志一字不差。
#[tokio::test]
async fn replaying_while_the_session_appends_loses_and_repeats_nothing() {
    const EARLY: usize = 15;
    const TURNS: usize = 30;
    let home = Home::new();
    let script = Script::new((0..TURNS).map(|_| Play::Says("好。")));
    let core = home.core(&script);
    let mut talker = Client::connect(Arc::clone(&core));
    talker.hello().await;
    let session = talker.create("c0", "~").await;
    // 先攒一段日志，补的时候有得读。
    for n in 0..EARLY {
        talker.say(&format!("a{n}"), &session, "hi").await;
        home.until_turns(&session, n + 1).await;
    }
    let mut followers = Vec::new();
    for n in EARLY..TURNS {
        if n % 5 == 0 {
            followers.push(tokio::spawn(follow(
                Arc::clone(&core),
                session.clone(),
                TURNS,
            )));
        }
        talker.say(&format!("a{n}"), &session, "hi").await;
        home.until_turns(&session, n + 1).await;
    }
    let log = logged(&home, &session);
    for follower in followers {
        let (seen, reply) = follower.await.expect("没 panic");
        let upto = reply["result"]["upto"].as_u64().expect("回应带 upto");
        assert!(upto <= log.len() as u64, "{reply}");
        assert_eq!(seen, log, "补的和推的合起来就是日志：upto {upto}");
    }
}
