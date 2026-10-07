//! 会话列表的推送（施工 9-5，`docs/blueprint/protocol.md`「会话列表的推送」）：订阅的回应是整张列表；造会话、一轮开始、空下来、
//! 改名、置顶、内核起的标题、删会话各推一项，整项和 `session.list` 的那一项一样；没标题的带第一句话的 `preview`；取消以后
//! 不推；参数不对的不订阅。
//!
//! 一个连接订阅列表、只看推送，另一个连接造会话、说话、改名：推送不靠发命令的那个连接。等推送一律有上限（十秒）。

mod support;

use std::time::Duration;

use serde_json::{Value, json};

use miyu_session::testkit::{Play, Script};

use support::*;

/// 连上、握手、订阅会话列表，交回连接和回应里的列表。
async fn watching(home: &Home, script: &Script) -> (Client, Client, Vec<Value>) {
    let core = home.core(script);
    let mut watcher = Client::connect(core.clone());
    watcher.hello().await;
    let reply = watcher
        .call("w1", "subscribe", json!({"stream": "sessions"}))
        .await;
    let sessions = reply["result"]["sessions"]
        .as_array()
        .unwrap_or_else(|| panic!("回应里有列表：{reply}"))
        .clone();
    let mut actor = Client::connect(core);
    actor.hello().await;
    (watcher, actor, sessions)
}

/// 读推送，直到会话 `session` 有一项合 `wanted` 的，交回那一项（`removed` 的交回整个 `params`）。
async fn until_entry(
    watcher: &mut Client,
    session: &str,
    wanted: impl Fn(&Value) -> bool,
) -> Value {
    loop {
        let next = watcher.next().await.expect("十秒内推了");
        assert_eq!(next["method"], "sessions.changed", "{next}");
        let params = &next["params"];
        if params["session"] != json!(session) {
            continue;
        }
        let item = if params["removed"] == json!(true) {
            params.clone()
        } else {
            params["entry"].clone()
        };
        if wanted(&item) {
            return item;
        }
    }
}

/// `session.list` 里会话 `session` 那一项：订阅着列表的连接上，回应之前可能有推送，跳过它们。
async fn listed(client: &mut Client, id: &str, session: &str) -> Value {
    let request = json!({"jsonrpc": "2.0", "id": id, "method": "session.list", "params": {}});
    client.line(&request.to_string()).await;
    let (_, reply) = client.until_reply(id).await;
    reply["result"]["sessions"]
        .as_array()
        .expect("有会话列表")
        .iter()
        .find(|item| item["session"] == json!(session))
        .cloned()
        .unwrap_or(Value::Null)
}

#[tokio::test]
async fn subscribing_gives_the_whole_list_with_previews() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。")]);
    let core = home.core(&script);
    let mut client = Client::connect(core.clone());
    client.hello().await;
    let work = home.work.to_string_lossy().into_owned();
    let session = client.create("c1", &work).await;
    client
        .say("c2", &session, "\n  帮我看看 CI 为什么红了  \n第二行")
        .await;
    home.until_turns(&session, 1).await;
    // 日志里有了 `turn.ended`，会话这一批可能还没送完、还算忙：等它空下来，两次列出来的才一样。
    for n in 0..1000 {
        if listed(&mut client, &format!("b{n}"), &session)
            .await
            .get("busy")
            .is_none()
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let reply = client
        .call("s1", "subscribe", json!({"stream": "sessions"}))
        .await;
    let listed = listed(&mut client, "l1", &session).await;
    assert_eq!(reply["result"]["sessions"], json!([listed]), "{reply}");
    assert_eq!(listed["preview"], "帮我看看 CI 为什么红了");
    assert!(listed.get("title").is_none());
    for (n, params) in [
        json!({"stream": "sessions", "session": session}),
        json!({"stream": "sessions", "after": 0}),
    ]
    .into_iter()
    .enumerate()
    {
        let id = format!("s{}", n + 2);
        let request = json!({"jsonrpc": "2.0", "id": id, "method": "subscribe", "params": params});
        client.line(&request.to_string()).await;
        let (_, reply) = client.until_reply(&id).await;
        assert_eq!(reply["error"]["data"]["reason"], "bad_params", "{reply}");
    }
}

#[tokio::test]
async fn every_change_pushes_the_whole_entry() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。")]);
    let (mut watcher, mut actor, sessions) = watching(&home, &script).await;
    assert_eq!(sessions, Vec::<Value>::new(), "还没有会话");
    let work = home.work.to_string_lossy().into_owned();
    let session = actor.create("c1", &work).await;
    let created = until_entry(&mut watcher, &session, |_| true).await;
    assert_eq!(created["session"], json!(session));
    assert!(created.get("busy").is_none(), "{created}");
    actor.say("c2", &session, "hi").await;
    until_entry(&mut watcher, &session, |item| item["busy"] == json!(true)).await;
    let idle = until_entry(&mut watcher, &session, |item| item.get("busy").is_none()).await;
    assert_eq!(idle["preview"], "hi");
    assert_eq!(
        idle,
        listed(&mut actor, "l1", &session).await,
        "和 session.list 那一项一样"
    );
    let reply = actor
        .call(
            "m1",
            "session.set_meta",
            json!({"session": session, "title": "修 CI"}),
        )
        .await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    let titled = until_entry(&mut watcher, &session, |item| item["title"] == "修 CI").await;
    assert!(
        titled.get("preview").is_none(),
        "有标题就不带预览：{titled}"
    );
    let reply = actor
        .call(
            "m2",
            "session.set_meta",
            json!({"session": session, "pinned": true}),
        )
        .await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    let pinned = until_entry(&mut watcher, &session, |item| item["pinned"] == json!(true)).await;
    assert_eq!(pinned, listed(&mut actor, "l2", &session).await);
    let reply = actor
        .call("d1", "session.delete", json!({"session": session}))
        .await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    let removed = until_entry(&mut watcher, &session, |item| {
        item["removed"] == json!(true)
    })
    .await;
    assert_eq!(removed, json!({"session": session, "removed": true}));
}

#[tokio::test]
async fn a_title_the_kernel_gives_is_pushed_too() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。")]).titles([Play::Says("修 CI")]);
    let (mut watcher, mut actor, _) = watching(&home, &script).await;
    let work = home.work.to_string_lossy().into_owned();
    let session = actor.create("c1", &work).await;
    actor.say("c2", &session, "CI 红了").await;
    let titled = until_entry(&mut watcher, &session, |item| item["title"] == "修 CI").await;
    assert_eq!(titled, listed(&mut actor, "l1", &session).await);
}

#[tokio::test]
async fn unsubscribing_stops_the_pushes() {
    let home = Home::new();
    let script = Script::new([]);
    let (mut watcher, mut actor, _) = watching(&home, &script).await;
    let reply = watcher
        .call("w2", "unsubscribe", json!({"stream": "sessions"}))
        .await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    let work = home.work.to_string_lossy().into_owned();
    actor.create("c1", &work).await;
    // 造好了、落了盘才回应；推送这时早该到了，等一会儿确认没有。
    let next = watcher.next_within(Duration::from_millis(300)).await;
    assert!(next.is_none(), "取消了不推：{next:?}");
}

/// 回应以后推来的都不比回应里的旧（`protocol.md`「会话列表的推送」第 4 条）：另一个连接一直在改名，这时订阅，回应里的标题
/// 是第 N 个，之后推来的只能是 N 或者 N 以后的，照先后。同一个推两次可以：列表算的时候索引已经换上了，会话报「变了」稍晚
/// 一步进队，整项替换不怕重。
#[tokio::test]
async fn nothing_older_than_the_reply_comes_after_it() {
    let home = Home::new();
    let script = Script::new([]);
    let core = home.core(&script);
    let mut actor = Client::connect(core.clone());
    actor.hello().await;
    let work = home.work.to_string_lossy().into_owned();
    let session = actor.create("c1", &work).await;
    let renaming = {
        let session = session.clone();
        tokio::spawn(async move {
            for n in 0..200 {
                let params = json!({"session": session, "title": format!("t{n}")});
                let reply = actor
                    .call(&format!("m{n}"), "session.set_meta", params)
                    .await;
                assert_eq!(reply["result"], json!({}), "{reply}");
            }
        })
    };
    let number = |title: &Value| -> u64 {
        title
            .as_str()
            .and_then(|title| title.strip_prefix('t'))
            .and_then(|n| n.parse().ok())
            .unwrap_or(0)
    };
    let mut watcher = Client::connect(core);
    watcher.hello().await;
    tokio::time::sleep(Duration::from_millis(5)).await;
    let reply = watcher
        .call("w1", "subscribe", json!({"stream": "sessions"}))
        .await;
    let mut last = number(&reply["result"]["sessions"][0]["title"]);
    while last < 199 {
        // 推到第 199 个就停：照先后、不往回走。
        let entry = until_entry(&mut watcher, &session, |_| true).await;
        let now = number(&entry["title"]);
        assert!(now >= last, "回应以后推来了旧的：{now} 在 {last} 前面");
        last = now;
    }
    renaming.await.expect("改名的任务没 panic");
}
