//! 列会话，真核心走一遍（施工 C-3，`docs/blueprint/cross-session.md` 第一条）：`session.list` 每一项的工作目录跟着头报的换、
//! 忙着的写 `busy`；她在一个主会话里调 `sessions`，列出同一个属主的别的主会话，不列她自己、她派的子代理、删了的，和
//! `session.list` 是同一份。
//!
//! 几个会话同时请求模型，谁先到不一定：替身照请求里人这边的那句分给各自的剧本（`support/deleting.rs` 的 `Router`）。

mod support;

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::SessionId;
use miyu_kernel::template::escape;
use miyu_session::testkit::{Play, Script};

use support::deleting::*;
use support::*;

/// `session.list` 里会话 `session` 那一项。
fn item<'a>(listed: &'a Value, session: &str) -> Option<&'a Value> {
    listed["result"]["sessions"]
        .as_array()
        .expect("有会话列表")
        .iter()
        .find(|item| item["session"] == json!(session))
}

/// 日志里最后一次调用交回的那一段字。
fn last_result(log: &[Event]) -> String {
    let result = log
        .iter()
        .rev()
        .find_map(|event| match &event.body {
            Body::ToolResult(result) => Some(result),
            _ => None,
        })
        .expect("有一次调用");
    match result.blocks.as_slice() {
        [Block::Text(Text { text })] => text.clone(),
        other => panic!("一段字：{other:?}"),
    }
}

fn short(session: &str) -> String {
    SessionId::parse(session)
        .expect("合写法")
        .short()
        .to_string()
}

#[tokio::test]
async fn she_lists_the_other_main_sessions_the_way_the_head_does() {
    let home = Home::new();
    // B 停在请求上（一直忙着）；A 派一个子代理，子代理也停着；A 再列会话。
    let router = Router(Arc::new(vec![
        ("子会话的交代", Script::new([Play::Holds])),
        (
            "我还有哪些会话",
            Script::new([Play::calls(&[("sessions", "{}")]), Play::Says("好。")]),
        ),
        (
            "派一个",
            Script::new([
                Play::calls(&[("subagent", &agent("子会话的交代"))]),
                Play::Says("派了。"),
            ]),
        ),
        ("忙着", Script::new([Play::Holds])),
    ]));
    let mut client = Client::connect(home.core_with_models(Arc::new(router), base_tools()));
    client.hello().await;
    let work = home.work.to_string_lossy().into_owned();
    let sub = home.work.join("sub");
    std::fs::create_dir_all(&sub).expect("建得了");
    let sub = sub.to_string_lossy().into_owned();

    // B：起了标题，头换到 `sub` 里说了一句，停在请求上。
    let b = client.create("c1", &work).await;
    let reply = client
        .call(
            "m1",
            "session.set_meta",
            json!({"session": b, "title": "修 CI"}),
        )
        .await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    let reply = client
        .call(
            "s1",
            "session.send",
            json!({"session": b, "text": "忙着", "cwd": sub}),
        )
        .await;
    assert!(reply["result"].is_object(), "{reply}");
    // D：删掉了。
    let d = client.create("c2", &work).await;
    let (_, reply) = delete(&mut client, "d1", &d).await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    // A：派出去一个子代理。
    let a = client.create("c3", &work).await;
    client.say("s2", &a, "派一个").await;
    home.until_turns(&a, 1).await;
    let child = started_child(&home.log(&a)).expect("派出去了子代理");

    // `session.list`：B 的工作目录是后来报的、忙着；A 闲着；子会话挂在 A 下面；D 不在。
    let mut n = 0;
    let listed = loop {
        n += 1;
        let listed = client
            .call(&format!("l{n}"), "session.list", json!({}))
            .await;
        let busy = |id: &str| item(&listed, id).is_some_and(|item| item["busy"] == json!(true));
        // A 的那一轮落了盘，actor 放下忙的记号还要一会儿。
        if busy(&b) && busy(child.as_str()) && !busy(&a) {
            break listed;
        }
        assert!(n < 2000, "B 和子会话一直不忙、A 一直忙：{listed}");
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    };
    let item_b = item(&listed, &b).expect("有 B");
    assert_eq!(item_b["cwd"], json!(sub));
    assert_eq!(item_b["title"], json!("修 CI"));
    let log_b = home.log(&b);
    assert_eq!(
        item_b["last_active"],
        json!(log_b.last().expect("有日志").at),
        "最近一次动静是日志最后一条的时刻"
    );
    let item_a = item(&listed, &a).expect("有 A");
    assert_eq!(item_a["cwd"], json!(work));
    assert!(item_a.get("busy").is_none(), "闲着的不写：{item_a}");
    assert_eq!(
        item(&listed, child.as_str()).expect("有子会话")["parent"],
        json!(a)
    );
    assert!(item(&listed, &d).is_none(), "删了的不列");

    // 她在 A 里列会话：第一行是 A，下面只有 B。
    client.say("s3", &a, "我还有哪些会话").await;
    home.until_turns(&a, 2).await;
    let text = last_result(&home.log(&a));
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2, "{text}");
    assert_eq!(lines[0], format!("You are session {}.", short(&a)));
    let row = format!(
        "{} \"修 CI\" in {}: busy, last active ",
        short(&b),
        escape(&sub)
    );
    assert!(lines[1].starts_with(&row), "{text}");
    for other in [child.as_str(), &d] {
        assert!(!text.contains(&short(other)), "不列 {other}：{text}");
    }
}
