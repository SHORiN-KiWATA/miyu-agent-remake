//! 订阅事件流（`docs/construction/3-8-协议端点（中）.md` 验收第 2 条）：先见结果、后见回应；两个会话不串；
//! 取消订阅以后不再推；读得慢的掉队，推一条 `resync`；取消订阅时已经交给转发任务的回应照样到（施工 4-9 再补三上）。

use std::time::Duration;

use serde_json::json;

use crate::support::{Client, Home, apart_from_now, kinds, reason};
use miyu_session::testkit::{Play, Script};

#[tokio::test]
async fn pushes_come_before_the_reply() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([Play::Says("你好。")])));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let reply = client.subscribe("c2", &session).await;
    assert_eq!(
        apart_from_now(&reply["result"]),
        json!({"limits": {}, "model": {"endpoint": "deepseek", "model": "deepseek-v4"}, "preset": "full"}),
        "{reply}"
    );

    client
        .line(&json!({"jsonrpc": "2.0", "id": "c3", "method": "session.send", "params": {"session": session, "text": "hi"}}).to_string())
        .await;
    let (before, reply) = client.until_reply("c3").await;
    // 回应到的时候，这一句产生的事件已经推过来了：人的消息在回应之前。
    let events = reply["result"]["events"]
        .as_array()
        .expect("接受了")
        .clone();
    let seqs: Vec<_> = before
        .iter()
        .map(|push| push["params"]["event"]["seq"].clone())
        .collect();
    assert!(
        events.iter().all(|seq| seqs.contains(seq)),
        "{events:?} 应该在回应之前推过来：{seqs:?}"
    );
    assert!(
        kinds(&before).contains(&"message.user".to_string()),
        "{:?}",
        kinds(&before)
    );
    assert!(
        before
            .iter()
            .all(|push| push["params"]["session"] == json!(session))
    );

    // 接着是增量、她的回复、回合结束。
    let after = client.until_turn_ends(&session).await;
    let kinds = kinds(&after);
    for kind in ["model.delta", "message.assistant", "turn.ended"] {
        assert!(
            kinds.contains(&kind.to_string()),
            "{kind} 应该在 {kinds:?} 里"
        );
    }
}

#[tokio::test]
async fn two_sessions_do_not_mix() {
    let home = Home::new();
    let script = Script::new([Play::Says("一。"), Play::Says("二。")]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let a = client.create("c1", "~").await;
    let b = client.create("c2", "~").await;
    client.subscribe("c3", &a).await;
    client.subscribe("c4", &b).await;
    client.say("c5", &a, "hi").await;
    let pushed = client.until_turn_ends(&a).await;
    assert!(
        pushed
            .iter()
            .filter(|push| push["method"] == json!("event"))
            .all(|push| push["params"]["session"] == json!(a)),
        "说给 a 的，推的都是 a 的"
    );
    client.say("c6", &b, "hi").await;
    let pushed = client.until_turn_ends(&b).await;
    assert!(
        pushed
            .iter()
            .filter(|push| push["method"] == json!("event"))
            .all(|push| push["params"]["session"] == json!(b)),
        "说给 b 的，推的都是 b 的"
    );
}

#[tokio::test]
async fn an_unsubscribed_session_pushes_nothing() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([Play::Says("好。")])));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.subscribe("c2", &session).await;
    let reply = client
        .call(
            "c3",
            "unsubscribe",
            json!({"session": session, "stream": "events"}),
        )
        .await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    let reply = client.say("c4", &session, "hi").await;
    assert!(reply["result"]["events"].is_array(), "{reply}");
    home.until_turns(&session, 1).await;
    assert_eq!(
        client.next_within(Duration::from_millis(200)).await,
        None,
        "取消了订阅，不再推"
    );
}

#[tokio::test]
async fn a_slow_reader_gets_a_resync_and_every_reply() {
    let home = Home::new();
    // 几千段增量：远多于写队列（256 行）、会话的推送队列（1024 份），再加管道里放得下的。
    let script = Script::new([Play::Floods(5000), Play::Says("好。"), Play::Says("好。")]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.subscribe("c2", &session).await;
    let send = |id: &str, text: &str| {
        json!({"jsonrpc": "2.0", "id": id, "method": "session.send", "params": {"session": session, "text": text}})
            .to_string()
    };
    client.line(&send("c3", "hi")).await;
    // 先不读：等这一轮在磁盘上说完。
    home.until_turns(&session, 1).await;
    // 推送堵着的时候再说一句：它的回应排在这个订阅的转发任务里，转发任务掉队停下时也要放出来。
    client.line(&send("c4", "again")).await;
    home.until_turns(&session, 2).await;
    let (mut resynced, mut after, mut replies) = (false, 0, Vec::new());
    while let Some(next) = client.next_within(Duration::from_millis(500)).await {
        if next["method"] == json!("resync") {
            assert_eq!(
                next["params"],
                json!({"session": session, "stream": "events"})
            );
            resynced = true;
        } else if next["method"] == json!("event") {
            if resynced {
                after += 1;
            }
        } else {
            replies.push(next["id"].clone());
        }
    }
    assert!(resynced, "读得慢的收到一条 resync");
    assert_eq!(after, 0, "掉了队，这个订阅停了");
    assert_eq!(replies, [json!("c3"), json!("c4")], "回应一条都不丢");

    // 掉了队、还没重新订阅：说一句，回应照样到，只是不推了。
    let reply = client
        .call(
            "c5",
            "session.interrupt",
            json!({"session": session, "queued": "return"}),
        )
        .await;
    assert_eq!(reason(&reply), Some("not_running"), "{reply}");

    // 重新订阅：照常推。
    let reply = client.subscribe("c6", &session).await;
    assert_eq!(
        apart_from_now(&reply["result"]),
        json!({"limits": {}, "model": {"endpoint": "deepseek", "model": "deepseek-v4"}, "preset": "full"}),
        "{reply}"
    );
    client.say("c7", &session, "once more").await;
    let pushed = client.until_turn_ends(&session).await;
    assert!(kinds(&pushed).contains(&"message.assistant".to_string()));
}

#[tokio::test]
async fn the_reply_waits_behind_a_backlog() {
    let home = Home::new();
    // 几百段增量：够堵满写队列和管道，又不到会话那边掉队的份数。转发任务卡在写队列上的时候，回应要是
    // 直接写，就插到这一句产生的推送前面了。
    let script = Script::new([Play::Floods(800), Play::Says("好。")]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.subscribe("c2", &session).await;
    let send = |id: &str, text: &str| {
        json!({"jsonrpc": "2.0", "id": id, "method": "session.send", "params": {"session": session, "text": text}})
            .to_string()
    };
    client.line(&send("c3", "hi")).await;
    home.until_turns(&session, 1).await;
    // 堵着的时候说第二句。
    client.line(&send("c4", "again")).await;
    home.until_turns(&session, 2).await;
    let mut seen = Vec::new();
    let reply = loop {
        let next = client.next().await.expect("没断开");
        assert_ne!(next["method"], json!("resync"), "这么几百段不该掉队");
        if next["id"] == json!("c4") {
            break next;
        }
        seen.push(next["params"]["event"]["seq"].clone());
    };
    let events = reply["result"]["events"].as_array().expect("接受了");
    assert!(
        events.iter().all(|seq| seen.contains(seq)),
        "第二句产生的事件 {events:?} 应该在它的回应之前推过来"
    );
}

/// 取消订阅时，已经交给转发任务、还没写出去的回应照样到（施工 4-9 再补三上：原来当场掐掉转发任务，回应丢了）。
/// 几百段增量把写队列和管道堵满，转发任务卡在写队列上；堵着的时候说第二句，它的回应排在转发任务手里；这时取消，
/// 再慢慢读。还订阅着的再订阅一次，是同一个订阅，回应也照样到。
#[tokio::test]
async fn unsubscribing_keeps_the_replies_already_handed_over() {
    for method in ["unsubscribe", "subscribe"] {
        let home = Home::new();
        let script = Script::new([Play::Floods(800), Play::Says("好。")]);
        let mut client = Client::connect(home.core(&script));
        client.hello().await;
        let session = client.create("c1", "~").await;
        client.subscribe("c2", &session).await;
        let send = |id: &str, text: &str| {
            json!({"jsonrpc": "2.0", "id": id, "method": "session.send", "params": {"session": session, "text": text}})
                .to_string()
        };
        client.line(&send("c3", "hi")).await;
        home.until_turns(&session, 1).await;
        client.line(&send("c4", "again")).await;
        home.until_turns(&session, 2).await;
        let stop = json!({"jsonrpc": "2.0", "id": "c5", "method": method, "params": {"session": session, "stream": "events"}});
        client.line(&stop.to_string()).await;
        let mut replies = Vec::new();
        while let Some(next) = client.next_within(Duration::from_millis(500)).await {
            assert_ne!(next["method"], json!("resync"), "这么几百段不该掉队");
            if next.get("id").is_some() {
                replies.push(next["id"].clone());
            }
        }
        replies.sort_by_key(ToString::to_string);
        assert_eq!(
            replies,
            [json!("c3"), json!("c4"), json!("c5")],
            "{method}：回应一条都不丢"
        );
    }
}

/// 订阅着的会话停了：推一条 `resync`，头知道这个订阅没了（施工 4-9 再补三上：原来静静地断）。
#[tokio::test]
async fn a_session_that_stops_sends_a_resync() {
    let home = Home::new();
    // 端口一叫就 panic：这个会话的 actor 停了。
    let script = Script::new([Play::Panics]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.subscribe("c2", &session).await;
    let send = json!({"jsonrpc": "2.0", "id": "c3", "method": "session.send", "params": {"session": session, "text": "hi"}});
    client.line(&send.to_string()).await;
    let mut resync = None;
    while let Some(next) = client.next_within(Duration::from_secs(60)).await {
        if next["method"] == json!("resync") {
            resync = Some(next);
            break;
        }
    }
    let resync = resync.expect("会话停了，推了 resync");
    assert_eq!(
        resync["params"],
        json!({"session": session, "stream": "events"})
    );
}

#[tokio::test]
async fn subscribing_needs_hello_a_session_and_a_known_stream() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    let missing = "0192f3a0-1111-7abc-8def-001122334455";
    let reply = client.subscribe("c1", missing).await;
    assert_eq!(reason(&reply), Some("hello_first"), "{reply}");
    client.hello().await;
    let reply = client.subscribe("c2", missing).await;
    assert_eq!(reason(&reply), Some("session_not_found"), "{reply}");
    let session = client.create("c3", "~").await;
    let reply = client
        .call(
            "c4",
            "subscribe",
            json!({"session": session, "stream": "view"}),
        )
        .await;
    assert_eq!(
        reason(&reply),
        Some("bad_params"),
        "现在只有事件流：{reply}"
    );
}
