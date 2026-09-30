//! 订阅时补发之前的事件（施工 3-8 六补，`docs/blueprint/protocol.md` 的 `subscribe`）：写了 `after` 的，先补日志里序号
//! 大于它、落了盘的，都在回应前面，回应带 `upto`；不写的照旧；写错的 `bad_params`；瞬时事件不补；要载入的会话照样补；
//! 已经订阅着的换一个新的。掉了队带 `after` 补回来、补的同时会话在追加，在 `replay_race.rs`。

mod support;

use std::time::Duration;

use serde_json::{Value, json};

use miyu_session::testkit::{Play, Script};
use support::{Client, Home, events, kinds, logged, reason};

/// 造一个会话，说完一轮，交回会话编号和磁盘上的日志。
async fn one_turn(home: &Home, client: &mut Client) -> (String, Vec<Value>) {
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("c2", &session, "hi").await;
    home.until_turns(&session, 1).await;
    let log = logged(home, &session);
    (session, log)
}

#[tokio::test]
async fn after_zero_replays_the_whole_log_before_the_reply() {
    let home = Home::new();
    // 一大串增量：瞬时的，不补。
    let script = Script::new([Play::Floods(50), Play::Says("好。")]);
    let mut client = Client::connect(home.core(&script));
    let (session, log) = one_turn(&home, &mut client).await;
    let (pushed, reply) = client.subscribe_after("c3", &session, json!(0)).await;
    assert_eq!(events(&pushed), log, "补的就是磁盘上的，一字不差");
    assert_eq!(pushed.len(), log.len(), "只有补的事件：{pushed:?}");
    assert!(
        pushed
            .iter()
            .all(|push| push["params"]["session"] == json!(session))
    );
    assert!(
        !kinds(&pushed).contains(&"model.delta".to_string()),
        "瞬时的不补"
    );
    let last = log.last().expect("有事件")["seq"].clone();
    assert_eq!(
        reply["result"],
        json!({"limits": {}, "upto": last}),
        "{reply}"
    );

    // 补完接着推新的：从下一条起。
    client
        .line(&json!({"jsonrpc": "2.0", "id": "c4", "method": "session.send", "params": {"session": session, "text": "again"}}).to_string())
        .await;
    let live = events(&client.until_turn_ends(&session).await);
    let persisted: Vec<_> = live
        .iter()
        .filter(|event| event.get("seq").is_some())
        .collect();
    assert_eq!(
        persisted.first().expect("推了")["seq"],
        json!(last.as_u64().expect("整数") + 1)
    );
}

#[tokio::test]
async fn after_a_middle_seq_replays_only_what_follows() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([Play::Says("好。")])));
    let (session, log) = one_turn(&home, &mut client).await;
    for after in [1, 3, log.len() - 1] {
        let id = format!("s{after}");
        let (pushed, reply) = client.subscribe_after(&id, &session, json!(after)).await;
        assert_eq!(events(&pushed), log[after..], "after {after}");
        assert_eq!(reply["result"]["upto"], json!(log.len()), "{reply}");
    }
}

#[tokio::test]
async fn after_the_last_seq_replays_nothing_and_goes_on_live() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。"), Play::Says("又好。")]);
    let mut client = Client::connect(home.core(&script));
    let (session, log) = one_turn(&home, &mut client).await;
    for (id, after) in [("c3", log.len()), ("c4", log.len() + 5)] {
        let (pushed, reply) = client.subscribe_after(id, &session, json!(after)).await;
        assert_eq!(pushed, Vec::<Value>::new(), "什么都不补");
        assert_eq!(
            reply["result"],
            json!({"limits": {}, "upto": log.len()}),
            "upto 是日志里最后一条：{reply}"
        );
    }
    client
        .line(&json!({"jsonrpc": "2.0", "id": "c5", "method": "session.send", "params": {"session": session, "text": "again"}}).to_string())
        .await;
    let live = events(&client.until_turn_ends(&session).await);
    let seqs: Vec<_> = live
        .iter()
        .filter_map(|event| event["seq"].as_u64())
        .collect();
    let total = logged(&home, &session).len() as u64;
    assert_eq!(
        seqs,
        (log.len() as u64 + 1..=total).collect::<Vec<_>>(),
        "照常推新的，一条不重"
    );
}

#[tokio::test]
async fn without_after_nothing_is_replayed() {
    for after in [None, Some(Value::Null)] {
        let home = Home::new();
        let mut client = Client::connect(home.core(&Script::new([Play::Says("好。")])));
        let (session, _) = one_turn(&home, &mut client).await;
        let mut params = json!({"session": session, "stream": "events"});
        if let Some(after) = after {
            params["after"] = after;
        }
        let reply = client.call("c3", "subscribe", params).await;
        assert_eq!(
            reply["result"],
            json!({"limits": {}}),
            "照旧，没有 upto：{reply}"
        );
        assert_eq!(
            client.next_within(Duration::from_millis(200)).await,
            None,
            "以前的不补"
        );
    }
}

#[tokio::test]
async fn a_bad_after_is_bad_params_and_subscribes_nothing() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。"), Play::Says("又好。")]);
    let mut client = Client::connect(home.core(&script));
    let (session, _) = one_turn(&home, &mut client).await;
    // 写成原文：2 的 64 次方放不进 `serde_json::Value` 的整数。
    let bad = [
        "-1",
        "1.5",
        "1.0",
        "\"3\"",
        "true",
        "[1]",
        "{\"seq\":1}",
        "18446744073709551616",
    ];
    for (n, after) in bad.into_iter().enumerate() {
        let line = format!(
            r#"{{"jsonrpc":"2.0","id":"b{n}","method":"subscribe","params":{{"session":"{session}","stream":"events","after":{after}}}}}"#
        );
        client.line(&line).await;
        let reply = client.next().await.expect("有回应");
        assert_eq!(reason(&reply), Some("bad_params"), "{after}：{reply}");
    }
    // 先查参数，不找会话。
    let missing = "0192f3a0-1111-7abc-8def-001122334455";
    let params = json!({"session": missing, "stream": "events", "after": -1});
    let reply = client.call("b-missing", "subscribe", params).await;
    assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
    // 一个都没订阅上：说一句，只有回应，不推。
    let reply = client.say("c9", &session, "again").await;
    assert!(reply["result"]["events"].is_array(), "{reply}");
    home.until_turns(&session, 2).await;
    assert_eq!(client.next_within(Duration::from_millis(200)).await, None);
}

/// 核心重启以后，会话不在表里：带 `after` 订阅，先载入，再照样补。
#[tokio::test]
async fn a_session_that_must_be_loaded_is_replayed_too() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。")]);
    let first = home.core(&script);
    let mut client = Client::connect(first.clone());
    let (session, _) = one_turn(&home, &mut client).await;
    first.stop_sessions().await;
    drop(client);
    let log = logged(&home, &session);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let (pushed, reply) = client.subscribe_after("c3", &session, json!(2)).await;
    assert_eq!(events(&pushed), log[2..]);
    assert_eq!(reply["result"]["upto"], json!(log.len()), "{reply}");
}

/// 已经订阅着、推送堵着、手里还有回应的时候带 `after` 再订阅：换一个新的。旧的先把交给它的推送、回应都写完，补的才
/// 开始，不插在中间；之后只有新的在推，一条不重。
#[tokio::test]
async fn subscribing_again_with_after_replaces_the_live_one() {
    let home = Home::new();
    // 几百段增量：够堵满写队列和管道，又不到会话那边掉队的份数（和 `subscribe.rs` 一样）。
    let script = Script::new([Play::Floods(800), Play::Says("好。")]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.subscribe("c2", &session).await;
    client
        .line(&json!({"jsonrpc": "2.0", "id": "c3", "method": "session.send", "params": {"session": session, "text": "hi"}}).to_string())
        .await;
    home.until_turns(&session, 1).await;
    // 堵着的时候再发几条：回应都排在旧的转发任务手里，它要先把攒着的推送放完才放得出这几条。
    for id in ["i1", "i2", "i3"] {
        let interrupt = json!({"jsonrpc": "2.0", "id": id, "method": "session.interrupt", "params": {"session": session, "queued": "return"}});
        client.line(&interrupt.to_string()).await;
    }
    let log = logged(&home, &session);
    let params = json!({"session": session, "stream": "events", "after": 0});
    let subscribe = json!({"jsonrpc": "2.0", "id": "c4", "method": "subscribe", "params": params});
    client.line(&subscribe.to_string()).await;
    // 先不读：要是不等旧的放完，补的这时已经排在写队列门口，和旧的挨个轮着进去。
    tokio::time::sleep(Duration::from_millis(300)).await;
    let (pushed, reply) = client.until_reply("c4").await;
    assert!(
        pushed.iter().all(|push| push["method"] != json!("resync")),
        "这么几百段不该掉队"
    );
    for id in ["c3", "i1", "i2", "i3"] {
        assert!(
            pushed.iter().any(|push| push["id"] == json!(id)),
            "旧的手里的回应 {id} 照样到"
        );
    }
    // 补的从 `session.created` 起：旧的订阅是造完会话才有的，推不出它。
    let start = pushed
        .iter()
        .position(|push| push["params"]["event"]["kind"] == json!("session.created"))
        .expect("补了");
    assert_eq!(
        pushed[start..].to_vec(),
        events(&pushed[start..])
            .into_iter()
            .map(|event| json!({"jsonrpc": "2.0", "method": "event", "params": {"session": session, "event": event}}))
            .collect::<Vec<_>>(),
        "补的中间没有夹着旧的推送、回应"
    );
    assert_eq!(events(&pushed[start..]), log);
    assert_eq!(reply["result"]["upto"], json!(log.len()), "{reply}");

    client
        .line(&json!({"jsonrpc": "2.0", "id": "c5", "method": "session.send", "params": {"session": session, "text": "again"}}).to_string())
        .await;
    let pushed = client.until_turn_ends(&session).await;
    let seqs: Vec<_> = events(&pushed)
        .iter()
        .filter_map(|event| event["seq"].as_u64())
        .collect();
    let total = logged(&home, &session).len() as u64;
    assert_eq!(
        seqs,
        (log.len() as u64 + 1..=total).collect::<Vec<_>>(),
        "只有新的在推，一条不重"
    );
}

/// 日志读不了（第一行被弄坏了）：带 `after` 订阅是 `session_broken`，什么都没订阅上。没有要补的（`after` 不比最后一条小）
/// 不读日志，照常订阅上。
#[tokio::test]
async fn an_unreadable_log_refuses_only_when_there_is_something_to_replay() {
    use std::io::{Seek, SeekFrom, Write};

    let home = Home::new();
    let script = Script::new([Play::Says("好。"), Play::Says("又好。")]);
    let mut client = Client::connect(home.core(&script));
    let (session, log) = one_turn(&home, &mut client).await;
    let id = miyu_kernel::id::SessionId::parse(&session).expect("合写法");
    let segment = std::fs::read_dir(home.root.session_dir(&support::alice(), &id))
        .expect("会话目录在")
        .map(|entry| entry.expect("读得了").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "jsonl"))
        .min()
        .expect("有一段日志");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .open(&segment)
        .expect("开得了");
    file.seek(SeekFrom::Start(0)).expect("挪得了");
    file.write_all(b"x").expect("写得进");
    drop(file);

    let (pushed, reply) = client.subscribe_after("c3", &session, json!(0)).await;
    assert_eq!(reason(&reply), Some("session_broken"), "{reply}");
    assert!(pushed.is_empty(), "{pushed:?}");
    let reply = client.say("c4", &session, "again").await;
    assert!(
        reply["result"]["events"].is_array(),
        "没订阅上，回应直接到：{reply}"
    );
    // 磁盘上的日志读不了，等不到这一轮说完：等一会儿，这一轮的推送一条都不来。
    assert_eq!(client.next_within(Duration::from_millis(500)).await, None);

    // 没有要补的：不读日志，照常订阅上。
    let after = json!(log.len() + 1000);
    let (pushed, reply) = client.subscribe_after("c5", &session, after).await;
    assert!(pushed.is_empty(), "{pushed:?}");
    assert!(reply["result"]["upto"].is_u64(), "{reply}");
}
