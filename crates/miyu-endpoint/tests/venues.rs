//! 场所会话与外部身份（施工 O-3，`docs/blueprint/venues.md`）：主人对应表认出私聊里的本人；`venue.session` 造、找回（核心重启
//! 以后照样）、参数不对、该归系统账号的回 `no_system_account`；`as` 记成本人（带 `via`）或者外部身份（带 `account`、`role`）；
//! 本机的会话不收 `as`，场所会话只收带 `as` 的；场所会话不进 `session.list`、会话列表的推送；同一个命令编号再发只生效一次。

mod support;

use std::time::Duration;

use serde_json::{Value, json};

use miyu_kernel::event::Body;
use miyu_kernel::origin::{By, External, Person, Role};
use miyu_session::testkit::{Play, Script};

use miyu_tool::Catalog;

use support::deleting::base_tools;
use support::venues::bound_core;
use support::*;

/// 照对应表起来的核心（`support::venues`），连上、握手。
async fn connected(home: &Home, script: &Script) -> Client {
    connected_with(home, script, Catalog::default()).await
}

/// 同 [`connected`]，工具目录是 `tools`。
async fn connected_with(home: &Home, script: &Script, tools: Catalog) -> Client {
    let mut client = Client::connect(bound_core(home, script, tools));
    client.hello().await;
    client
}

async fn venue(client: &mut Client, id: &str, params: Value) -> Value {
    client.call(id, "venue.session", params).await
}

fn private(peer: &str) -> Value {
    json!({"venue": format!("qq:private:{}", &peer[3..]), "kind": "private", "peer": peer})
}

/// 日志里 `message.user` 的 `by`，照先后。
fn speakers(home: &Home, session: &str) -> Vec<By> {
    home.log(session)
        .into_iter()
        .filter(|event| matches!(event.body, Body::MessageUser(_)))
        .map(|event| event.by)
        .collect()
}

async fn send(client: &mut Client, id: &str, params: Value) -> Value {
    client.call(id, "session.send", params).await
}

#[tokio::test]
async fn the_owners_private_chat_is_found_again_and_kept_off_the_local_list() {
    let home = Home::new();
    let script = Script::new([]);
    let mut client = connected(&home, &script).await;
    let made = venue(&mut client, "v1", private("qq:10001")).await;
    assert_eq!(made["result"]["created"], true, "{made}");
    let session = made["result"]["session"]
        .as_str()
        .expect("有编号")
        .to_string();
    let again = venue(&mut client, "v2", private("qq:10001")).await;
    assert_eq!(
        again["result"],
        json!({"session": session, "created": false})
    );
    let listed = client.call("l1", "session.list", json!({})).await;
    assert_eq!(
        listed["result"]["sessions"],
        json!([]),
        "场所会话不在本机的列表里"
    );
    // 核心重启以后照索引找回同一个。
    let script = Script::new([]);
    let mut client = connected(&home, &script).await;
    let again = venue(&mut client, "v3", private("qq:10001")).await;
    assert_eq!(
        again["result"],
        json!({"session": session, "created": false})
    );
}

#[tokio::test]
async fn venues_that_need_a_system_account_and_bad_params_are_refused() {
    let home = Home::new();
    let script = Script::new([]);
    let mut client = connected(&home, &script).await;
    for (n, params) in [
        private("qq:20002"),
        private("qq:10003"),
        json!({"venue": "qq:group:1", "kind": "group"}),
    ]
    .into_iter()
    .enumerate()
    {
        let reply = venue(&mut client, &format!("n{n}"), params).await;
        assert_eq!(reason(&reply), Some("no_system_account"), "{reply}");
    }
    for (n, params) in [
        json!({"venue": "qq:private:1", "kind": "private"}),
        json!({"venue": "qq:group:1", "kind": "group", "peer": "qq:1"}),
        json!({"venue": "qq:group:1", "kind": "room"}),
        json!({"venue": "", "kind": "group"}),
    ]
    .into_iter()
    .enumerate()
    {
        let reply = venue(&mut client, &format!("b{n}"), params).await;
        assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
    }
    let listed = client.call("l1", "session.list", json!({})).await;
    assert_eq!(listed["result"]["sessions"], json!([]), "什么都没造");
}

#[tokio::test]
async fn who_speaks_in_a_venue_session() {
    let home = Home::new();
    let script = Script::new([Play::Says("在。"), Play::Says("嗯。"), Play::Says("好。")]);
    let mut client = connected(&home, &script).await;
    let made = venue(&mut client, "v1", private("qq:10001")).await;
    let session = made["result"]["session"]
        .as_str()
        .expect("有编号")
        .to_string();
    let reply = send(
        &mut client,
        "s0",
        json!({"session": session, "text": "在吗"}),
    )
    .await;
    assert_eq!(
        reason(&reply),
        Some("venue_session"),
        "不带 as 的不收：{reply}"
    );
    let reply = send(
        &mut client,
        "s1",
        json!({"session": session, "text": "在吗", "as": {"external": "qq:10001"}}),
    )
    .await;
    assert!(reply["result"]["events"].is_array(), "{reply}");
    home.until_turns(&session, 1).await;
    for (id, speaker) in [
        ("s2", json!({"external": "qq:20002"})),
        ("s3", json!({"external": "qq:10003", "role": "manager"})),
    ] {
        let reply = send(
            &mut client,
            id,
            json!({"session": session, "text": "hi", "as": speaker}),
        )
        .await;
        assert!(reply["result"]["events"].is_array(), "{reply}");
    }
    home.until_turns(&session, 3).await;
    let venue = miyu_kernel::id::VenueId::parse("qq:private:10001").expect("合写法");
    let mut owner = Person::new(alice());
    owner.via = Some(miyu_kernel::id::ExternalId::parse("qq:10001").expect("合写法"));
    assert_eq!(
        speakers(&home, &session),
        [
            By::Person(owner),
            By::External(External {
                venue: venue.clone(),
                id: miyu_kernel::id::ExternalId::parse("qq:20002").expect("合写法"),
                account: None,
                role: Some(Role::Member),
            }),
            By::External(External {
                venue,
                id: miyu_kernel::id::ExternalId::parse("qq:10003").expect("合写法"),
                account: None,
                role: Some(Role::Manager),
            }),
        ],
        "对着不存在的账号的照没写"
    );
}

#[tokio::test]
async fn local_sessions_do_not_take_as_and_as_does_not_mix_with_from() {
    let home = Home::new();
    let script = Script::new([]);
    let mut client = connected(&home, &script).await;
    let work = home.work.to_string_lossy().into_owned();
    let local = client.create("c1", &work).await;
    let reply = send(
        &mut client,
        "s1",
        json!({"session": local, "text": "hi", "as": {"external": "qq:10001"}}),
    )
    .await;
    assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
    let made = venue(&mut client, "v1", private("qq:10001")).await;
    assert_eq!(
        made["result"]["created"], true,
        "本机的会话不当成场所会话找回：{made}"
    );
    let session = made["result"]["session"]
        .as_str()
        .expect("有编号")
        .to_string();
    let reply = send(
        &mut client,
        "s2",
        json!({"session": session, "text": "hi", "as": {"external": "qq:10001"}, "from": "x"}),
    )
    .await;
    assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
    assert!(speakers(&home, &session).is_empty(), "什么都没送");
}

#[tokio::test]
async fn the_same_command_id_counts_once_even_after_a_restart() {
    let home = Home::new();
    let script = Script::new([Play::Says("在。")]);
    let mut client = connected(&home, &script).await;
    let made = venue(&mut client, "v1", private("qq:10001")).await;
    let session = made["result"]["session"]
        .as_str()
        .expect("有编号")
        .to_string();
    let said = json!({"session": session, "text": "在吗", "as": {"external": "qq:10001"}});
    let first = send(&mut client, "qq:bot:msg-1", said.clone()).await;
    home.until_turns(&session, 1).await;
    let again = send(&mut client, "qq:bot:msg-1", said.clone()).await;
    assert_eq!(
        again["result"]["events"], first["result"]["events"],
        "{again}"
    );
    let script = Script::new([]);
    let mut client = connected(&home, &script).await;
    let again = send(&mut client, "qq:bot:msg-1", said).await;
    // 重启以后回应和头一次一样（施工 2-1 补统一了）。
    assert_eq!(
        again["result"]["events"], first["result"]["events"],
        "重启以后照样认得：{again}"
    );
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(speakers(&home, &session).len(), 1, "只记了一次");
}

#[tokio::test]
async fn venue_sessions_are_not_pushed_to_the_local_list() {
    let home = Home::new();
    let script = Script::new([]);
    let mut client = connected(&home, &script).await;
    let reply = client
        .call("w1", "subscribe", json!({"stream": "sessions"}))
        .await;
    assert_eq!(reply["result"]["sessions"], json!([]), "{reply}");
    let request = json!({"jsonrpc": "2.0", "id": "v1", "method": "venue.session", "params": private("qq:10001")});
    client.line(&request.to_string()).await;
    let (mut pushed, _) = client.until_reply("v1").await;
    let work = home.work.to_string_lossy().into_owned();
    let request =
        json!({"jsonrpc": "2.0", "id": "c1", "method": "session.create", "params": {"cwd": work}});
    client.line(&request.to_string()).await;
    let (before, reply) = client.until_reply("c1").await;
    pushed.extend(before);
    let local = reply["result"]["session"].clone();
    // 推来的只有本机的那一个：造它的推送可能在回应前面，也可能在后面。
    if pushed.is_empty() {
        pushed.push(client.next().await.expect("推了"));
    }
    let sessions: Vec<&Value> = pushed
        .iter()
        .filter(|push| push["method"] == "sessions.changed")
        .map(|push| &push["params"]["session"])
        .collect();
    assert_eq!(sessions, [&local], "{pushed:?}");
}

/// 她用 `sessions` 列会话时，场所会话不在里面，`history` 也就认不出它（施工 O-3）。
#[tokio::test]
async fn her_sessions_tool_does_not_list_venue_sessions() {
    let home = Home::new();
    let script = Script::new([Play::calls(&[("sessions", "{}")]), Play::Says("好。")]);
    let mut client = connected_with(&home, &script, base_tools()).await;
    let made = venue(&mut client, "v1", private("qq:10001")).await;
    let venue_session = made["result"]["session"]
        .as_str()
        .expect("有编号")
        .to_string();
    let work = home.work.to_string_lossy().into_owned();
    let local = client.create("c1", &work).await;
    client.say("s1", &local, "列一下").await;
    home.until_turns(&local, 1).await;
    let listed = home
        .log(&local)
        .into_iter()
        .find_map(|event| match event.body {
            Body::ToolResult(result) => Some(format!("{:?}", result.blocks)),
            _ => None,
        })
        .expect("调了 sessions");
    let short = miyu_kernel::id::SessionId::parse(&venue_session)
        .expect("合写法")
        .short()
        .to_string();
    assert!(!listed.contains(&short), "场所会话不列：{listed}");
}
