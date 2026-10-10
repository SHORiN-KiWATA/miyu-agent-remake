//! 订阅记忆日志（施工 R-12 上，`docs/blueprint/memory.md`「协议」，`protocol.md` 的流 `memory`）：真核心、真记忆日志。
//!
//! 带 `after` 的先补那以后的、再回应 `upto`、再推新追加的，不丢不重；不带的只推之后的；推的是日志里那一行原样；别的人格
//! 的不推；再订阅换掉、取消了不推；读得慢的掉队推 `resync`；人格记忆没装的、没有记忆的照原因拒；参数写错的不对。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::memories::{connected, with_persona};
use crate::support::*;

/// 推送比回应先到的，收在 `early` 里，`next_push` 照先后先交它们。
struct Watch {
    client: Client,
    early: std::collections::VecDeque<Value>,
}

impl Watch {
    /// 发一个请求，等到回应：先到的推送收起来。
    async fn call(&mut self, id: &str, method: &str, params: Value) -> Value {
        let line = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        self.client.line(&line.to_string()).await;
        let (pushed, reply) = self.client.until_reply(id).await;
        self.early.extend(pushed);
        reply
    }

    async fn remember(&mut self, id: &str, text: &str) {
        let reply = self
            .call(
                id,
                "memory.remember",
                json!({"class": "user", "text": text}),
            )
            .await;
        assert!(reply["result"]["id"].is_string(), "{reply}");
    }

    /// 下一条推送（先交收起来的，再等最多一秒）。
    async fn next_push(&mut self) -> Option<Value> {
        match self.early.pop_front() {
            Some(early) => Some(early),
            None => self.client.next_within(Duration::from_secs(1)).await,
        }
    }
}

fn watch(client: Client) -> Watch {
    Watch {
        client,
        early: std::collections::VecDeque::new(),
    }
}

/// 发 `subscribe`，收到回应为止：交回之前收到的推送和回应。
async fn follow(watch: &mut Watch, id: &str, params: Value) -> (Vec<Value>, Value) {
    let mut params = params;
    params["stream"] = json!("memory");
    let line = json!({"jsonrpc": "2.0", "id": id, "method": "subscribe", "params": params});
    watch.client.line(&line.to_string()).await;
    watch.client.until_reply(id).await
}

/// 推送里那一行事件：方法、带的那一格、事件的序号和种类。
fn pushed(message: &Value) -> (String, u64, String) {
    assert_eq!(message["method"], "memory.event", "{message}");
    (
        message["params"]["persona"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        message["params"]["event"]["seq"].as_u64().expect("有序号"),
        message["params"]["event"]["kind"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
    )
}

#[tokio::test]
async fn after_fills_in_then_replies_then_follows() {
    let home = Home::new();
    let mut client = watch(connected(&home, &Script::new([])).await);
    client.remember("c1", "用户养了一只猫").await;
    client.remember("c2", "回答要短").await;
    let (filled, reply) = follow(
        &mut client,
        "s1",
        json!({"persona": "engineer", "after": 0}),
    )
    .await;
    let filled: Vec<(String, u64, String)> = filled.iter().map(pushed).collect();
    assert_eq!(
        filled,
        [
            ("engineer".into(), 1, "ext.memory.saved".into()),
            ("engineer".into(), 2, "ext.memory.saved".into()),
        ],
        "先补"
    );
    assert_eq!(reply["result"], json!({"upto": 2}), "{reply}");

    client.remember("c3", "用户住在杭州").await;
    let message = client.next_push().await.expect("推了");
    assert_eq!(
        pushed(&message),
        ("engineer".into(), 3, "ext.memory.saved".into())
    );
    let event = &message["params"]["event"];
    assert_eq!(
        event["body"]["text"], "用户住在杭州",
        "日志里那一行原样：{event}"
    );
    assert_eq!(event["by"]["kind"], "person");
    assert_eq!(event["cause"], "c3");

    let forgot = client
        .call("c4", "memory.forget", json!({"id": "m1", "why": "不养了"}))
        .await;
    assert_eq!(forgot["result"], json!({}), "{forgot}");
    let message = client.next_push().await.expect("推了");
    assert_eq!(
        pushed(&message),
        ("engineer".into(), 4, "ext.memory.retired".into())
    );
    assert_eq!(
        message["params"]["event"]["body"],
        json!({"id": "m1", "why": "不养了"})
    );
}

#[tokio::test]
async fn without_after_only_what_comes_next_and_from_the_middle() {
    let home = Home::new();
    let core = with_persona(&home, &Script::new([]), Catalog::default());
    let mut client = Client::connect(core.clone());
    client.hello().await;
    let mut client = watch(client);
    client.remember("c1", "用户养了一只猫").await;
    client.remember("c2", "回答要短").await;
    let (filled, reply) = follow(&mut client, "s1", json!({"persona": "engineer"})).await;
    assert!(filled.is_empty(), "不写 after 的不补：{filled:?}");
    assert_eq!(reply["result"], json!({"upto": 2}));
    client.remember("c3", "用户住在杭州").await;
    assert_eq!(pushed(&client.next_push().await.expect("推了")).1, 3);

    let mut other = Client::connect(core);
    other.hello().await;
    let mut other = watch(other);
    let (filled, reply) =
        follow(&mut other, "s2", json!({"persona": "engineer", "after": 2})).await;
    let seqs: Vec<u64> = filled.iter().map(|message| pushed(message).1).collect();
    assert_eq!(seqs, [3], "只补 2 以后的");
    assert_eq!(reply["result"], json!({"upto": 3}));
}

/// 她在聊天里调 `remember` 记的，走会话那条路、不经协议：订阅着的照样收到，`by` 是那次调用、出处是那个会话。
#[tokio::test]
async fn what_she_remembers_in_a_chat_is_pushed_too() {
    let home = Home::new();
    let script = Script::new([
        Play::calls(&[(
            "remember",
            &json!({"class": "user", "text": "用户养了一只猫"}).to_string(),
        )]),
        Play::Says("记住了。"),
    ]);
    let resources = default_resources();
    let mut tools = miyu_basesystem::tools(&resources).expect("读得出");
    tools.extend(miyu_memory::tools(&resources).expect("读得出"));
    let core = with_persona(&home, &script, Catalog::new(tools).expect("合写法"));
    let mut watcher = Client::connect(Arc::clone(&core));
    watcher.hello().await;
    let mut watcher = watch(watcher);
    follow(&mut watcher, "s1", json!({"persona": "engineer"})).await;
    let mut talker = Client::connect(core);
    talker.hello().await;
    let session = talker.create("c1", "~").await;
    talker.say("c2", &session, "我养了一只猫").await;
    home.until_turns(&session, 1).await;
    let message = watcher.next_push().await.expect("推了");
    assert_eq!(
        pushed(&message),
        ("engineer".into(), 1, "ext.memory.saved".into())
    );
    let event = &message["params"]["event"];
    assert_eq!(event["by"]["kind"], "tool", "{event}");
    assert_eq!(event["body"]["sources"][0]["session"], session.as_str());
}

#[tokio::test]
async fn another_rooms_and_unsubscribed_ones_are_quiet() {
    let home = Home::new();
    let mut client = watch(connected(&home, &Script::new([])).await);
    let (_, reply) = follow(&mut client, "s1", json!({"persona": "engineer"})).await;
    assert_eq!(reply["result"], json!({"upto": 0}), "一条都没有的是 0");
    // 记忆只在会话里的会话：它那一间在会话目录里，和人格那一间不相干。
    let created = client
        .call(
            "c0",
            "session.create",
            json!({"cwd": "~", "memory": "session"}),
        )
        .await;
    let session = created["result"]["session"]
        .as_str()
        .expect("造了")
        .to_string();
    let (_, room) = follow(&mut client, "s2", json!({"session": session})).await;
    assert_eq!(room["result"], json!({"upto": 0}), "{room}");
    let elsewhere = client
        .call(
            "c1",
            "memory.remember",
            json!({"session": session, "class": "user", "text": "只在这个会话里的"}),
        )
        .await;
    assert!(elsewhere["result"]["id"].is_string(), "{elsewhere}");
    let message = client.next_push().await.expect("会话那一间的推了");
    assert_eq!(message["method"], "memory.event", "{message}");
    assert_eq!(
        message["params"]["session"],
        session.as_str(),
        "带的是订阅时写的那一格"
    );
    assert!(message["params"].get("persona").is_none(), "{message}");
    assert!(client.next_push().await.is_none(), "人格那一间的不推");
    let gone = client
        .call(
            "u0",
            "unsubscribe",
            json!({"stream": "memory", "session": session}),
        )
        .await;
    assert_eq!(gone["result"], json!({}));

    let gone = client
        .call(
            "u1",
            "unsubscribe",
            json!({"stream": "memory", "persona": "engineer"}),
        )
        .await;
    assert_eq!(gone["result"], json!({}));
    client.remember("c2", "取消以后记的").await;
    assert!(client.next_push().await.is_none(), "取消了不推");
    let again = client
        .call(
            "u2",
            "unsubscribe",
            json!({"stream": "memory", "persona": "engineer"}),
        )
        .await;
    assert_eq!(again["result"], json!({}), "没订阅着的也回空的");
}

#[tokio::test]
async fn a_second_subscription_replaces_the_first() {
    let home = Home::new();
    let mut client = watch(connected(&home, &Script::new([])).await);
    follow(&mut client, "s1", json!({"persona": "engineer"})).await;
    client.remember("c1", "用户养了一只猫").await;
    assert_eq!(pushed(&client.next_push().await.expect("推了")).1, 1);
    let (filled, _) = follow(
        &mut client,
        "s2",
        json!({"persona": "engineer", "after": 0}),
    )
    .await;
    let seqs: Vec<u64> = filled.iter().map(|message| pushed(message).1).collect();
    assert_eq!(seqs, [1], "新的那一个照样补");
    client.remember("c2", "回答要短").await;
    assert_eq!(pushed(&client.next_push().await.expect("推了")).1, 2);
    assert!(client.next_push().await.is_none(), "换掉了：只推一份");
}

#[tokio::test]
async fn a_slow_reader_gets_a_resync_and_every_reply() {
    let home = Home::new();
    // 几千段增量把这个连接的写队列堵上：记忆的推送就放不进去，掉队。
    let script = Script::new([Play::Floods(5000)]);
    let core = with_persona(&home, &script, Catalog::default());
    let mut slow = Client::connect(Arc::clone(&core));
    slow.hello().await;
    let session = slow.create("c1", "~").await;
    slow.subscribe("c2", &session).await;
    let followed = json!({"jsonrpc": "2.0", "id": "c3", "method": "subscribe",
        "params": {"stream": "memory", "persona": "engineer"}});
    slow.line(&followed.to_string()).await;
    let (_, reply) = slow.until_reply("c3").await;
    assert_eq!(reply["result"], json!({"upto": 0}), "{reply}");
    let send = json!({"jsonrpc": "2.0", "id": "c4", "method": "session.send",
        "params": {"session": session, "text": "hi"}});
    slow.line(&send.to_string()).await;
    home.until_turns(&session, 1).await;
    let mut other = watch(Client::connect(core));
    other.client.hello().await;
    for n in 0..80 {
        other
            .remember(&format!("o{n}"), &format!("第 {n} 条"))
            .await;
    }
    let (mut resynced, mut after, mut replies) = (false, 0, Vec::new());
    while let Some(next) = slow.next_within(Duration::from_millis(500)).await {
        match next["method"].as_str() {
            Some("resync") if next["params"]["stream"] == "memory" => {
                assert_eq!(
                    next["params"],
                    json!({"stream": "memory", "persona": "engineer"})
                );
                resynced = true;
            }
            Some("memory.event") if resynced => after += 1,
            Some(_) => {}
            None => replies.push(next["id"].clone()),
        }
    }
    assert!(resynced, "读得慢的收到一条记忆的 resync");
    assert_eq!(after, 0, "掉了队，这个订阅停了");
    assert_eq!(replies, [json!("c4")], "回应一条都不丢");
}

#[tokio::test]
async fn refusals() {
    let home = Home::new();
    let mut client = watch(connected(&home, &Script::new([])).await);
    for (params, why) in [
        (
            json!({"stream": "memory", "persona": "engineer", "after": -1}),
            "bad_params",
        ),
        (
            json!({"stream": "memory", "persona": "engineer", "session": "x"}),
            "bad_params",
        ),
        (
            json!({"stream": "memory", "persona": "nobody"}),
            "unknown_persona",
        ),
    ] {
        let reply = client.call("r", "subscribe", params.clone()).await;
        assert_eq!(reason(&reply), Some(why), "{params}：{reply}");
    }
    let created = client
        .call("c0", "session.create", json!({"cwd": "~", "memory": "off"}))
        .await;
    let off = created["result"]["session"].as_str().expect("造了");
    let reply = client
        .call(
            "r1",
            "subscribe",
            json!({"stream": "memory", "session": off}),
        )
        .await;
    assert_eq!(reason(&reply), Some("memory_unavailable"), "{reply}");
    let removed = client
        .call("p1", "package.remove", json!({"package": "memory"}))
        .await;
    assert_eq!(removed["result"]["removed"], true, "{removed}");
    let reply = client
        .call(
            "r2",
            "subscribe",
            json!({"stream": "memory", "persona": "engineer"}),
        )
        .await;
    assert_eq!(reason(&reply), Some("memory_not_installed"), "{reply}");
}
