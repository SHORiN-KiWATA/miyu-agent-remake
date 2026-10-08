//! 人格（施工 P-1 上，`docs/blueprint/personas.md`）：真核心走一遍。家目录里的人格叠在出厂的上面，人设进 system、示范对话
//! 排在 system 后面历史前面；没写人格的照 `persona.default`（个人设置压着系统配置）；`venue.session` 带人格造、找回时不看；
//! 没有的 `unknown_persona`，编号不合写法的参数不对，写错了的 `persona_invalid` 带问题；`persona.list`、`persona.get`；
//! 角色扮演提示排在人说的那句后面、system 带风格锁（施工 P-1 补）。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_kernel::block::Block;
use miyu_kernel::request::{Message, Request};
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::venues::{BINDINGS, configured_core};
use crate::support::*;

/// 管理员（测试里是 alice）家目录里的人格 `id` 的一个文件。
fn mine(home: &Home, id: &str, file: &str, text: &str) {
    home.write(&format!("home/alice/personas/{id}/{file}"), text);
}

/// 读系统配置、个人设置的核心（`support::venues`）。
fn configured(home: &Home, script: &Script) -> Arc<Core> {
    configured_core(home, script, Catalog::default())
}

async fn connected(core: Arc<Core>) -> Client {
    let mut client = Client::connect(core);
    client.hello().await;
    client
}

/// 一次请求里每条消息：谁、说了什么字。
fn said(request: &Request) -> Vec<(&'static str, String)> {
    let text = |blocks: &[Block]| {
        blocks
            .iter()
            .filter_map(|block| match block {
                Block::Text(text) => Some(text.text.clone()),
                _ => None,
            })
            .collect::<String>()
    };
    request
        .messages
        .iter()
        .map(|message| match message {
            Message::User { blocks } => ("user", text(blocks)),
            Message::Assistant { blocks } => ("assistant", text(blocks)),
            _ => ("other", String::new()),
        })
        .collect()
}

async fn create(client: &mut Client, id: &str, params: Value) -> Value {
    client.call(id, "session.create", params).await
}

#[tokio::test]
async fn a_persona_in_the_home_speaks_with_its_examples() {
    let home = Home::new();
    mine(&home, "miyu", "prompts/persona.md", "You are Miyu.\n");
    mine(
        &home,
        "miyu",
        "prompts/examples.md",
        "user: 在吗\nassistant: 在\n",
    );
    let script = Script::new([Play::Says("嗯。")]);
    let mut client = connected(configured(&home, &script)).await;
    let reply = create(&mut client, "c1", json!({"cwd": "~", "persona": "miyu"})).await;
    let session = reply["result"]["session"]
        .as_str()
        .expect("造出来了")
        .to_string();
    client.say("s1", &session, "hi").await;
    home.until_turns(&session, 1).await;
    let (_, request) = script.requests().into_iter().next().expect("发了请求");
    assert!(
        request.system.starts_with("You are Miyu."),
        "{}",
        request.system
    );
    assert_eq!(request.stable, 2);
    let said = said(&request);
    assert_eq!(
        said[..2],
        [
            ("user", "在吗".to_string()),
            ("assistant", "在".to_string())
        ]
    );
    let (who, last) = said.last().expect("有人说的");
    assert_eq!(*who, "user");
    assert!(
        last.ends_with("hi"),
        "历史里人说的那句在示范对话后面：{last}"
    );
}

/// 角色扮演提示（施工 P-1 补）：带着它的人格，第一轮的请求以它结尾（排在人说的那句后面），system 以风格锁结尾；第二轮
/// 不再注入，第一轮那一块照原样留着，前缀接得上。软件工程师两样都没有。
#[tokio::test]
async fn a_persona_with_a_reminder_gets_it_after_the_message_and_the_style_lock() {
    let home = Home::new();
    mine(&home, "miyu", "prompts/persona.md", "You are Miyu.\n");
    mine(&home, "miyu", "prompts/reminders.md", "Stay soft.\n\n");
    let script = Script::new([Play::Says("嗯。"), Play::Says("好。"), Play::Says("在。")]);
    let mut client = connected(configured(&home, &script)).await;
    let reply = create(&mut client, "c1", json!({"cwd": "~", "persona": "miyu"})).await;
    let session = reply["result"]["session"]
        .as_str()
        .expect("造出来了")
        .to_string();
    client.say("s1", &session, "hi").await;
    home.until_turns(&session, 1).await;
    client.say("s2", &session, "再说").await;
    home.until_turns(&session, 2).await;
    let requests = script.requests();
    let lock = include_str!("../../../resources/core/style-lock.txt");
    assert!(
        requests[0].1.system.ends_with(lock.trim_end()),
        "{}",
        requests[0].1.system
    );
    let first = said(&requests[0].1);
    let block = "<persona-reminder>\nStay soft.\n</persona-reminder>\n";
    assert!(first[0].1.ends_with(&format!("hi{block}")), "{first:?}");
    let second = said(&requests[1].1);
    assert_eq!(second[0], first[0], "第一轮那一块照原样留着");
    assert_eq!(
        second.last(),
        Some(&("user", "再说".to_string())),
        "{second:?}"
    );
    let reply = create(
        &mut client,
        "c2",
        json!({"cwd": "~", "persona": "engineer"}),
    )
    .await;
    let engineer = reply["result"]["session"]
        .as_str()
        .expect("造出来了")
        .to_string();
    client.say("s3", &engineer, "hi").await;
    home.until_turns(&engineer, 1).await;
    let plain = &script.requests()[2].1;
    assert!(!plain.system.contains("style-lock"), "{}", plain.system);
    assert!(!said(plain)[0].1.contains("persona-reminder"));
}

#[tokio::test]
async fn without_a_persona_the_default_is_used_personal_over_system() {
    let home = Home::new();
    mine(&home, "miyu", "prompts/persona.md", "You are Miyu.\n");
    mine(&home, "kiki", "prompts/persona.md", "You are Kiki.\n");
    let script = Script::new([Play::Says("a"), Play::Says("b"), Play::Says("c")]);
    // 都没写：出厂的软件工程师。
    let mut client = connected(configured(&home, &script)).await;
    let first = client.create("c1", "~").await;
    client.say("s1", &first, "hi").await;
    home.until_turns(&first, 1).await;
    // 系统配置写了 miyu。
    home.write("system/config.toml", "[persona]\ndefault = \"miyu\"\n");
    let mut client = connected(configured(&home, &script)).await;
    let second = client.create("c2", "~").await;
    client.say("s2", &second, "hi").await;
    home.until_turns(&second, 1).await;
    // 个人设置写了 kiki，压着系统配置。
    home.write(
        "home/alice/settings.toml",
        "[persona]\ndefault = \"kiki\"\n",
    );
    let mut client = connected(configured(&home, &script)).await;
    let third = client.create("c3", "~").await;
    client.say("s3", &third, "hi").await;
    home.until_turns(&third, 1).await;
    let systems: Vec<String> = script
        .requests()
        .into_iter()
        .map(|(_, request)| {
            request
                .system
                .lines()
                .next()
                .unwrap_or_default()
                .to_string()
        })
        .collect();
    assert_eq!(
        systems,
        [
            "You are a helpful software engineer.",
            "You are Miyu.",
            "You are Kiki."
        ]
    );
}

#[tokio::test]
async fn missing_bad_and_broken_personas_are_refused() {
    let home = Home::new();
    mine(&home, "broken", "prompts/examples.md", "user: a\nuser: b\n");
    let script = Script::new([]);
    let mut client = connected(configured(&home, &script)).await;
    let reply = create(&mut client, "c1", json!({"cwd": "~", "persona": "nobody"})).await;
    assert_eq!(reason(&reply), Some("unknown_persona"), "{reply}");
    let reply = create(&mut client, "c2", json!({"cwd": "~", "persona": "../x"})).await;
    assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
    let reply = create(&mut client, "c3", json!({"cwd": "~", "persona": "broken"})).await;
    assert_eq!(reason(&reply), Some("persona_invalid"), "{reply}");
    assert_eq!(
        reply["error"]["data"]["problem"],
        "home prompts/examples.md:2: user and assistant must take turns"
    );
    // 默认人格指着没有的：照样不悄悄换成别的。
    home.write("system/config.toml", "[persona]\ndefault = \"nobody\"\n");
    let mut client = connected(configured(&home, &script)).await;
    let reply = create(&mut client, "c4", json!({"cwd": "~"})).await;
    assert_eq!(reason(&reply), Some("unknown_persona"), "{reply}");
    let listed = client.call("l1", "session.list", json!({})).await;
    assert_eq!(listed["result"]["sessions"], json!([]), "什么都没造");
}

#[tokio::test]
async fn a_venue_session_is_made_with_the_given_persona_and_found_again_without_it() {
    let home = Home::new();
    home.write("system/config.toml", BINDINGS);
    mine(&home, "miyu", "prompts/persona.md", "You are Miyu.\n");
    let script = Script::new([Play::Says("在。")]);
    let mut client = connected(configured(&home, &script)).await;
    let private = json!({"venue": "qq:private:10001", "kind": "private", "peer": "qq:10001", "persona": "miyu"});
    let made = client.call("v1", "venue.session", private).await;
    let session = made["result"]["session"]
        .as_str()
        .expect("造出来了")
        .to_string();
    let again = json!({"venue": "qq:private:10001", "kind": "private", "peer": "qq:10001", "persona": "nobody"});
    let found = client.call("v2", "venue.session", again).await;
    assert_eq!(
        found["result"],
        json!({"session": session, "created": false}),
        "找回时不看人格"
    );
    let send = json!({"session": session, "text": "在吗", "as": {"external": "qq:10001"}});
    client.call("s1", "session.send", send).await;
    home.until_turns(&session, 1).await;
    let (_, request) = script.requests().into_iter().next().expect("发了请求");
    assert!(
        request.system.starts_with("You are Miyu."),
        "{}",
        request.system
    );
    // 新的场所指着没有的人格：不造。
    let other = json!({"venue": "qq:private:10001x", "kind": "private", "peer": "qq:10001", "persona": "nobody"});
    let reply = client.call("v3", "venue.session", other).await;
    assert_eq!(reason(&reply), Some("unknown_persona"), "{reply}");
}

#[tokio::test]
async fn personas_are_listed_and_read() {
    let home = Home::new();
    mine(
        &home,
        "miyu",
        "persona.toml",
        "[persona]\nname = { zh = \"美羽\", en = \"Miyu\" }\nsummary = { en = \"Mine.\" }\n",
    );
    mine(
        &home,
        "miyu",
        "prompts/examples.md",
        "user: a\nassistant: b\nuser: c\nassistant: d\n",
    );
    mine(
        &home,
        "engineer",
        "persona.toml",
        "[persona]\nsummary = { zh = \"我的工程师\" }\n",
    );
    mine(&home, "broken", "persona.toml", "[voice]\n");
    mine(&home, "miyu", "prompts/reminders.md", "Stay soft.\n");
    // 软件工程师只在家目录多一份提示：示范对话没有。
    mine(&home, "engineer", "prompts/reminders.md", "Stay.\n");
    let script = Script::new([]);
    let mut client = connected(configured(&home, &script)).await;
    let listed = client.call("l1", "persona.list", json!({})).await;
    assert_eq!(
        listed["result"]["personas"],
        json!([
            {"persona": "broken", "problem": "home persona.toml:1: unknown table [voice]"},
            {"persona": "engineer", "name": "软件工程师", "summary": "我的工程师"},
            {"persona": "miyu", "name": "美羽", "summary": "Mine."},
            {"persona": "none", "name": "空白", "summary": "不带人设，照原样说话"},
        ]),
        "握手说的是中文，挑中文，没有中文的照英文；来自哪几层不给（施工 P-3 补）"
    );
    let got = client
        .call("g1", "persona.get", json!({"persona": "miyu"}))
        .await;
    assert_eq!(
        got["result"],
        json!({
            "persona": "miyu",
            "name": "美羽",
            "summary": "Mine.",
            "prompts": {"persona": false, "reminders": true},
            "examples": 2,
            "remove": "delete",
        })
    );
    let got = client
        .call("g2", "persona.get", json!({"persona": "engineer"}))
        .await;
    assert_eq!(
        got["result"]["prompts"],
        json!({"persona": true, "reminders": true})
    );
    let got = client
        .call("g3", "persona.get", json!({"persona": "nobody"}))
        .await;
    assert_eq!(reason(&got), Some("unknown_persona"), "{got}");
    let got = client.call("g4", "persona.get", json!({})).await;
    assert_eq!(reason(&got), Some("bad_params"), "{got}");
}

/// 空人格和会话带上人格（施工 P-1 下）：选 `none` 的 system 里没有人设；`session.created`、会话列表、`subscribe` 的回应都写
/// 着用的是哪个人格。
#[tokio::test]
async fn the_empty_persona_says_nothing_and_every_view_names_the_persona() {
    let home = Home::new();
    let script = Script::new([Play::Says("嗯。"), Play::Says("好。")]);
    let mut client = connected(configured(&home, &script)).await;
    let reply = create(&mut client, "c1", json!({"cwd": "~", "persona": "none"})).await;
    let empty = reply["result"]["session"]
        .as_str()
        .expect("造出来了")
        .to_string();
    client.say("s1", &empty, "hi").await;
    home.until_turns(&empty, 1).await;
    let engineer = client.create("c2", "~").await;
    client.say("s2", &engineer, "hi").await;
    home.until_turns(&engineer, 1).await;
    let requests = script.requests();
    assert!(
        !requests[0].1.system.contains("software engineer"),
        "空人格不带人设：{}",
        requests[0].1.system
    );
    assert!(
        requests[1]
            .1
            .system
            .starts_with("You are a helpful software engineer.")
    );
    let created = home
        .log(&empty)
        .into_iter()
        .find_map(|event| match event.body {
            miyu_kernel::event::Body::SessionCreated(created) => Some(created),
            _ => None,
        })
        .expect("有造会话那一条");
    assert_eq!(created.persona.as_deref(), Some("none"));
    let listed = client.call("l1", "session.list", json!({})).await;
    let personas: Vec<(String, String)> = listed["result"]["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| {
            (
                item["session"].as_str().unwrap().to_string(),
                item["persona"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect();
    assert_eq!(
        personas,
        [
            (engineer.clone(), "engineer".to_string()),
            (empty.clone(), "none".to_string())
        ]
    );
    let subscribed = client.subscribe("w1", &empty).await;
    assert_eq!(subscribed["result"]["persona"], "none", "{subscribed}");
}

/// 以前的日志没有 `persona`：会话列表、`subscribe` 都不写这一格（施工 P-1 下）。
#[tokio::test]
async fn sessions_from_older_logs_name_no_persona() {
    let home = Home::new();
    let script = Script::new([Play::Says("嗯。")]);
    let mut client = connected(configured(&home, &script)).await;
    let session = client.create("c1", "~").await;
    client.say("s1", &session, "hi").await;
    home.until_turns(&session, 1).await;
    // 把造会话那一条改回以前的样子：去掉 persona。
    let dir = home.root.path().join("home/alice/sessions").join(&session);
    let first = dir.join("000000000001.jsonl");
    let text = std::fs::read_to_string(&first).unwrap();
    let old = text.replacen(",\"persona\":\"engineer\"", "", 1);
    assert_ne!(old, text, "写过 persona");
    std::fs::write(&first, old).unwrap();
    let script = Script::new([]);
    let mut client = connected(configured(&home, &script)).await;
    let subscribed = client.subscribe("w1", &session).await;
    assert!(
        subscribed["result"].get("persona").is_none(),
        "{subscribed}"
    );
    let listed = client.call("l1", "session.list", json!({})).await;
    assert!(
        listed["result"]["sessions"][0].get("persona").is_none(),
        "{listed}"
    );
}
