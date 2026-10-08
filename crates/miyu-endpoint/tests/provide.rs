//! 提供者（施工 O-2 上，`docs/construction/O-2-提供者（上）.md`）：核心拉起的扩展经 `provide` 登记工具，写错的、撞名的整个不收；
//! 本机的头不是提供者。新造的会话工具面里有给本机的那几件、没有只给群的；她调到时扩展收到 `tool.call`（带会话、调用编号、
//! 参数），回的结果、错误、写法不对的各自交回；扩展关掉了，下一个回合工具面里没有它的工具（施工 O-2 中），照旧调的说没有这件。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::Body;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::extensions::*;
use crate::support::*;

fn spec(name: &str, venues: Value) -> Value {
    json!({"name": name, "description": format!("The {name} tool."),
           "input_schema": {"type": "object"}, "access": "read", "venues": venues})
}

/// 等到 `path` 里记下的有 `n` 行，交回它们。
async fn lines(path: &std::path::Path, n: usize) -> Vec<String> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let got: Vec<String> = read(path).lines().map(str::to_string).collect();
        if got.len() >= n {
            return got;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "等不到 {n} 行：{got:?}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// 日志里每一条工具结果的字和出没出错，照先后。
fn results(home: &Home, session: &str) -> Vec<(String, bool)> {
    home.log(session)
        .into_iter()
        .filter_map(|event| match event.body {
            Body::ToolResult(result) => {
                let text = result
                    .blocks
                    .iter()
                    .map(|block| match block {
                        Block::Text(Text { text }) => text.clone(),
                        _ => String::new(),
                    })
                    .collect::<String>();
                Some((text, result.status != miyu_kernel::event::ToolStatus::Ok))
            }
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn an_extension_provides_tools_and_answers_the_calls() {
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "bridge");
    let doubled = json!({"tools": [spec("dup", json!(["local"])), spec("dup", json!(["local"]))]});
    let tools = json!({"tools": [
        spec("echo_back", json!(["local"])), spec("boom", json!(["local"])),
        spec("garbled", json!(["local"])), spec("oops", json!(["local"])),
        spec("group_only", json!(["group"])),
    ]});
    install(
        &home,
        "bridge",
        &program.name(),
        "always",
        &steps(&[
            &step,
            "hello",
            &format!("ask:provide:{doubled}"),
            &format!("ask:provide:{tools}"),
            "serve",
        ]),
    );
    let script = Script::new([
        Play::calls(&[
            ("echo_back", r#"{"x":1}"#),
            ("boom", "{}"),
            ("garbled", "{}"),
            ("oops", "{}"),
        ]),
        Play::Says("好。"),
        Play::calls(&[("echo_back", "{}")]),
        Play::Says("嗯。"),
    ]);
    let core = Arc::new(
        home.core_full(&script, Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    core.start_extensions();
    let got = lines(&path, 4).await;
    let doubled: Value = serde_json::from_str(&got[2]).unwrap();
    assert_eq!(doubled["error"]["data"]["reason"], "bad_tool", "{doubled}");
    assert_eq!(doubled["error"]["data"]["problem"], "duplicate");
    let provided: Value = serde_json::from_str(&got[3]).unwrap();
    assert_eq!(provided["result"], json!({"tools": 5}), "{provided}");

    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let refused = client.call("p1", "provide", json!({"tools": []})).await;
    assert_eq!(reason(&refused), Some("not_a_provider"), "{refused}");

    let session = client.create("c1", "~").await;
    client.say("s1", &session, "调一下").await;
    home.until_turns(&session, 1).await;
    let face: Vec<String> = script.requests()[0]
        .1
        .tools
        .iter()
        .map(|tool| tool.name.clone())
        .collect();
    assert_eq!(
        face,
        ["boom", "echo_back", "garbled", "oops"],
        "只给群的不在本机的会话里"
    );
    // 只读的几件一起跑，结果先到先记：照字排了再比。
    let mut answered = results(&home, &session);
    answered.sort();
    assert_eq!(
        answered,
        [
            ("boom".to_string(), true),
            ("oops".to_string(), true),
            ("served echo_back".to_string(), false),
            (r#"{"nothing":1}"#.to_string(), true),
        ]
    );
    let call = read(&path)
        .lines()
        .find(|line| line.contains(r#""tool":"echo_back""#))
        .expect("扩展收到了")
        .to_string();
    let call: Value = serde_json::from_str(&call).unwrap();
    assert_eq!(call["method"], "tool.call");
    assert_eq!(call["params"]["session"], json!(session));
    assert_eq!(call["params"]["args"], json!({"x": 1}));
    assert!(call["params"]["call_id"].is_string(), "{call}");

    // 关掉扩展：工具出目录，下一个回合拿掉（施工 O-2 中）；连接没来的暂时不可用见 `provide_later.rs`。
    let stopped = call_ext(&mut client, "extension.disable", "bridge").await;
    assert!(stopped.get("error").is_none(), "{stopped}");
    until_state(&mut client, "bridge", |entry| entry["state"] == "off").await;
    client.say("s2", &session, "再调一下").await;
    home.until_turns(&session, 2).await;
    assert_eq!(
        results(&home, &session).last(),
        Some(&("There is no tool named \"echo_back\".\n".to_string(), true))
    );
}

async fn call_ext(client: &mut Client, method: &str, id: &str) -> Value {
    call(client, method, id).await
}
