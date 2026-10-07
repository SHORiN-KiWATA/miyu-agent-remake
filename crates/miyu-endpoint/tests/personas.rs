//! 人格（施工 P-1 上，`docs/blueprint/personas.md`）：真核心走一遍。家目录里的人格叠在出厂的上面，人设进 system、示范对话
//! 排在 system 后面历史前面；没写人格的照 `persona.default`（个人设置压着系统配置）；`venue.session` 带人格造、找回时不看；
//! 没有的 `unknown_persona`，编号不合写法的参数不对，写错了的 `persona_invalid` 带问题；`persona.list`、`persona.get`。

mod support;

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_kernel::block::Block;
use miyu_kernel::request::{Message, Request};
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use support::venues::{BINDINGS, configured_core};
use support::*;

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
async fn personas_are_listed_and_read_by_layer() {
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
    let script = Script::new([]);
    let mut client = connected(configured(&home, &script)).await;
    let listed = client.call("l1", "persona.list", json!({})).await;
    assert_eq!(
        listed["result"]["personas"],
        json!([
            {"persona": "broken", "problem": "home persona.toml:1: unknown table [voice]"},
            {"persona": "engineer", "name": null, "summary": "我的工程师", "layers": ["shipped", "home"]},
            {"persona": "miyu", "name": "美羽", "summary": "Mine.", "layers": ["home"]},
        ]),
        "握手说的是中文，挑中文，没有中文的照英文"
    );
    let got = client
        .call("g1", "persona.get", json!({"persona": "miyu"}))
        .await;
    assert_eq!(
        got["result"],
        json!({
            "persona": "miyu",
            "name": {"en": "Miyu", "zh": "美羽"},
            "summary": {"en": "Mine."},
            "layers": ["home"],
            "prompts": {"persona": null, "examples": "home"},
            "examples": 2,
        })
    );
    let got = client
        .call("g2", "persona.get", json!({"persona": "engineer"}))
        .await;
    assert_eq!(got["result"]["prompts"]["persona"], "shipped");
    let got = client
        .call("g3", "persona.get", json!({"persona": "nobody"}))
        .await;
    assert_eq!(reason(&got), Some("unknown_persona"), "{got}");
    let got = client.call("g4", "persona.get", json!({})).await;
    assert_eq!(reason(&got), Some("bad_params"), "{got}");
}
