//! 改标题、置顶（施工 3-8 三补，`docs/blueprint/protocol.md` 的 `session.set_meta`）：改名、去标题、置顶、两样一起改，
//! 各记一条 `session.meta_changed`（只写改了的那几格）、推给订阅着的头，回应是 `{}`；和现在一样的什么都不记；两格都不写、
//! 标题空的、太长的、格的值不对是参数不对；没有的会话是找不到。`session.list` 每一项带上标题、置顶，核心重启以后照样。

mod support;

use serde_json::{Value, json};

use miyu_kernel::event::{Body, Event, MetaChanged};
use miyu_kernel::origin::{By, Person};
use miyu_session::testkit::{Play, Script};

use support::*;

/// 改标题、置顶，交回回应之前读到的推送和回应：订阅着的会话，推送排在回应前面。
async fn set_meta(client: &mut Client, id: &str, params: Value) -> (Vec<Value>, Value) {
    let request =
        json!({"jsonrpc": "2.0", "id": id, "method": "session.set_meta", "params": params});
    client.line(&request.to_string()).await;
    client.until_reply(id).await
}

/// 推送里的 `session.meta_changed` 的正文，照先后。
fn meta_pushes(pushed: &[Value]) -> Vec<Value> {
    pushed
        .iter()
        .filter(|push| push["params"]["event"]["kind"] == json!("session.meta_changed"))
        .map(|push| push["params"]["event"]["body"].clone())
        .collect()
}

/// 日志里的 `session.meta_changed`，和它那一条事件。
fn meta_events(log: &[Event]) -> Vec<(&Event, &MetaChanged)> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::MetaChanged(changed) => Some((event, changed)),
            _ => None,
        })
        .collect()
}

/// 造好一个会话、订阅着它，交回连接和会话编号。
async fn subscribed(home: &Home, script: &Script) -> (Client, String) {
    let mut client = Client::connect(home.core(script));
    client.hello().await;
    let work = home.work.to_string_lossy().into_owned();
    let session = client.create("c1", &work).await;
    client.subscribe("s1", &session).await;
    (client, session)
}

/// `session.list` 里会话 `session` 那一项。
async fn listed(client: &mut Client, id: &str, session: &str) -> Value {
    let reply = client.call(id, "session.list", json!({})).await;
    reply["result"]["sessions"]
        .as_array()
        .expect("有会话列表")
        .iter()
        .find(|item| item["session"] == json!(session))
        .cloned()
        .unwrap_or_else(|| panic!("列表里有 {session}：{reply}"))
}

#[tokio::test]
async fn renaming_records_the_trimmed_title_and_pushes_it_before_the_reply() {
    let home = Home::new();
    let (mut client, session) = subscribed(&home, &Script::new([])).await;
    let (pushed, reply) = set_meta(
        &mut client,
        "t1",
        json!({"session": session, "title": "  整理 src 目录 \n"}),
    )
    .await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    assert_eq!(meta_pushes(&pushed), [json!({"title": "整理 src 目录"})]);
    let log = home.log(&session);
    let metas = meta_events(&log);
    assert_eq!(metas.len(), 1);
    let (event, changed) = metas[0];
    assert_eq!(
        changed,
        &MetaChanged {
            title: Some("整理 src 目录".to_string()),
            pinned: None,
        },
        "只写改了的那一格，去掉前后空白"
    );
    assert_eq!(event.by, By::Person(Person::new(alice())));
    assert_eq!(event.cause.as_ref().map(|cause| cause.as_str()), Some("t1"));
    assert_eq!(event.turn, None, "空闲时改的不带回合");
}

#[tokio::test]
async fn pinning_unpinning_and_both_at_once() {
    let home = Home::new();
    let (mut client, session) = subscribed(&home, &Script::new([])).await;
    let (pushed, reply) = set_meta(
        &mut client,
        "p1",
        json!({"session": session, "pinned": true}),
    )
    .await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    assert_eq!(meta_pushes(&pushed), [json!({"pinned": true})]);
    let (pushed, _) = set_meta(
        &mut client,
        "p2",
        json!({"session": session, "pinned": false}),
    )
    .await;
    assert_eq!(meta_pushes(&pushed), [json!({"pinned": false})]);
    let (pushed, _) = set_meta(
        &mut client,
        "p3",
        json!({"session": session, "title": "发版", "pinned": true}),
    )
    .await;
    assert_eq!(
        meta_pushes(&pushed),
        [json!({"title": "发版", "pinned": true})],
        "两样一起改的记一条"
    );
    assert_eq!(meta_events(&home.log(&session)).len(), 3);
}

#[tokio::test]
async fn a_null_title_takes_the_title_away() {
    let home = Home::new();
    let (mut client, session) = subscribed(&home, &Script::new([])).await;
    set_meta(
        &mut client,
        "t1",
        json!({"session": session, "title": "发版"}),
    )
    .await;
    let (pushed, reply) = set_meta(
        &mut client,
        "t2",
        json!({"session": session, "title": null}),
    )
    .await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    assert_eq!(
        meta_pushes(&pushed),
        [json!({"title": ""})],
        "去掉标题记成空的标题"
    );
    let item = listed(&mut client, "l1", &session).await;
    assert!(item.get("title").is_none(), "去掉了就不写：{item}");
}

#[tokio::test]
async fn the_same_as_now_records_nothing() {
    let home = Home::new();
    let (mut client, session) = subscribed(&home, &Script::new([])).await;
    let before = home.log(&session).len();
    // 没有标题时去掉标题、没置顶时取消置顶：和现在一样。
    for (id, params) in [
        ("n1", json!({"session": session, "title": null})),
        ("n2", json!({"session": session, "pinned": false})),
        (
            "n3",
            json!({"session": session, "title": null, "pinned": false}),
        ),
    ] {
        let (pushed, reply) = set_meta(&mut client, id, params).await;
        assert_eq!(reply["result"], json!({}), "{id}: {reply}");
        assert!(meta_pushes(&pushed).is_empty(), "{id}");
    }
    set_meta(
        &mut client,
        "n4",
        json!({"session": session, "title": "发版", "pinned": true}),
    )
    .await;
    let after = home.log(&session).len();
    assert_eq!(after, before + 1);
    // 标题一样（前后空白不算）、置顶一样：什么都不记；一样的那一格也不写。
    for (id, params) in [
        ("n5", json!({"session": session, "title": " 发版 "})),
        ("n6", json!({"session": session, "pinned": true})),
        (
            "n7",
            json!({"session": session, "title": "发版", "pinned": true}),
        ),
    ] {
        let (pushed, reply) = set_meta(&mut client, id, params).await;
        assert_eq!(reply["result"], json!({}), "{id}: {reply}");
        assert!(meta_pushes(&pushed).is_empty(), "{id}");
    }
    assert_eq!(home.log(&session).len(), after, "一条都没多");
    let (pushed, _) = set_meta(
        &mut client,
        "n8",
        json!({"session": session, "title": "发版", "pinned": false}),
    )
    .await;
    assert_eq!(
        meta_pushes(&pushed),
        [json!({"pinned": false})],
        "只写变了的"
    );
}

#[tokio::test]
async fn titles_must_be_one_to_two_hundred_characters_after_trimming() {
    let home = Home::new();
    let (mut client, session) = subscribed(&home, &Script::new([])).await;
    let longest = "字".repeat(200);
    let (_, reply) = set_meta(
        &mut client,
        "l1",
        json!({"session": session, "title": longest}),
    )
    .await;
    assert_eq!(reply["result"], json!({}), "200 个字的收：{reply}");
    let padded = format!("  {}  ", "a".repeat(200));
    let (_, reply) = set_meta(
        &mut client,
        "l2",
        json!({"session": session, "title": padded}),
    )
    .await;
    assert_eq!(
        reply["result"],
        json!({}),
        "去掉空白以后 200 个的收：{reply}"
    );
    for (id, title) in [
        ("l3", "字".repeat(201)),
        ("l4", String::new()),
        ("l5", " \t\n ".to_string()),
    ] {
        let (_, reply) =
            set_meta(&mut client, id, json!({"session": session, "title": title})).await;
        assert_eq!(reason(&reply), Some("bad_params"), "{id}: {reply}");
    }
    assert_eq!(meta_events(&home.log(&session)).len(), 2);
}

#[tokio::test]
async fn wrong_params_are_refused_before_looking_for_the_session() {
    let home = Home::new();
    let (mut client, session) = subscribed(&home, &Script::new([])).await;
    let nobody = "01900000-0000-7000-8000-000000000000";
    for (id, params) in [
        ("b1", json!({"session": session})),
        ("b2", json!({"session": session, "pinned": null})),
        ("b3", json!({"session": nobody})),
        ("b4", json!({"session": session, "title": 3})),
        ("b5", json!({"session": session, "title": ["发版"]})),
        ("b6", json!({"session": session, "pinned": "yes"})),
        ("b7", json!({"session": "not-a-session", "pinned": true})),
        ("b8", json!({"pinned": true})),
    ] {
        let (_, reply) = set_meta(&mut client, id, params).await;
        assert_eq!(reason(&reply), Some("bad_params"), "{id}: {reply}");
    }
    let (_, reply) = set_meta(
        &mut client,
        "b9",
        json!({"session": nobody, "pinned": true}),
    )
    .await;
    assert_eq!(reason(&reply), Some("session_not_found"), "{reply}");
    assert_eq!(reply["error"]["message"], json!("没有这个会话。"));
    assert!(meta_events(&home.log(&session)).is_empty());
}

#[tokio::test]
async fn renaming_during_a_turn_carries_the_turn() {
    let home = Home::new();
    let (mut client, session) = subscribed(&home, &Script::new([Play::Holds])).await;
    client.say("m1", &session, "在吗").await;
    let (pushed, reply) = set_meta(
        &mut client,
        "t1",
        json!({"session": session, "title": "等着的"}),
    )
    .await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    assert_eq!(meta_pushes(&pushed), [json!({"title": "等着的"})]);
    let log = home.log(&session);
    let started = log
        .iter()
        .find(|event| matches!(event.body, Body::TurnStarted(_)))
        .map(|event| event.seq)
        .expect("开了一轮");
    let metas = meta_events(&log);
    assert_eq!(metas[0].0.turn.map(|turn| turn.started()), Some(started));
}

#[tokio::test]
async fn the_list_shows_titles_and_pins_even_after_a_restart() {
    let home = Home::new();
    let script = Script::new([]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let work = home.work.to_string_lossy().into_owned();
    let (a, b, c) = (
        client.create("c1", &work).await,
        client.create("c2", &work).await,
        client.create("c3", &work).await,
    );
    client
        .call(
            "t1",
            "session.set_meta",
            json!({"session": a, "title": "发版", "pinned": true}),
        )
        .await;
    client
        .call(
            "t2",
            "session.set_meta",
            json!({"session": b, "title": "旧名字"}),
        )
        .await;
    client
        .call(
            "t3",
            "session.set_meta",
            json!({"session": b, "title": "新名字", "pinned": true}),
        )
        .await;
    client
        .call(
            "t4",
            "session.set_meta",
            json!({"session": b, "pinned": false}),
        )
        .await;
    let check = |a_item: &Value, b_item: &Value, c_item: &Value| {
        assert_eq!(a_item["title"], json!("发版"));
        assert_eq!(a_item["pinned"], json!(true));
        assert_eq!(b_item["title"], json!("新名字"), "最后改成的");
        assert!(b_item.get("pinned").is_none(), "取消了置顶就不写：{b_item}");
        assert!(
            c_item.get("title").is_none() && c_item.get("pinned").is_none(),
            "{c_item}"
        );
    };
    let (a_item, b_item, c_item) = (
        listed(&mut client, "l1", &a).await,
        listed(&mut client, "l2", &b).await,
        listed(&mut client, "l3", &c).await,
    );
    check(&a_item, &b_item, &c_item);
    // 核心重启以后：照磁盘上的日志，会话一个都没载入。
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let (a_item, b_item, c_item) = (
        listed(&mut client, "l4", &a).await,
        listed(&mut client, "l5", &b).await,
        listed(&mut client, "l6", &c).await,
    );
    check(&a_item, &b_item, &c_item);
    // 载入以后照日志接着比：一样的不再记。
    let reply = client
        .call(
            "t5",
            "session.set_meta",
            json!({"session": a, "title": "发版", "pinned": true}),
        )
        .await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    assert_eq!(meta_events(&home.log(&a)).len(), 1, "载入以后照日志算回来");
}

/// 日志后面坏了的会话照样列出来，标题、置顶照坏的那一段以前的算（`protocol.md` 的 `session.list` 第 3 条）：只有一段的，
/// 算不出来，当没有；工作目录、最近一次动静照第一条（施工 C-3）。
#[tokio::test]
async fn a_log_broken_later_is_listed_as_far_as_it_reads() {
    let home = Home::new();
    let script = Script::new([]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let work = home.work.to_string_lossy().into_owned();
    let session = client.create("c1", &work).await;
    client
        .call(
            "t1",
            "session.set_meta",
            json!({"session": session, "title": "发版", "pinned": true}),
        )
        .await;
    let created_at = home.log(&session)[0].at;
    let id = miyu_kernel::id::SessionId::parse(&session).expect("合写法");
    let segment = home
        .root
        .session_dir(&alice(), &id)
        .join("000000000001.jsonl");
    let mut log = std::fs::read_to_string(&segment).expect("读得了");
    log.push_str("not an event\n");
    std::fs::write(&segment, log).expect("写得了");
    let item = listed(&mut client, "l1", &session).await;
    assert!(
        item.get("title").is_none() && item.get("pinned").is_none(),
        "{item}"
    );
    // 工作目录、最近一次动静照第一条算（施工 C-3）：只有一段、它坏了，后面的都不算。
    assert_eq!(item["cwd"], json!(work), "{item}");
    assert_eq!(item["last_active"], json!(created_at), "{item}");
}
