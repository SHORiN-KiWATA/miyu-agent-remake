//! 回答确认（施工 D-1，`docs/blueprint/protocol.md` 的 `session.answer`）：真核心走一遍。允许这一次的照常跑；本会话都允许的，
//! 同一个目录下一次写不再问；拒绝带的理由她看得到，这一轮接着走；参数不对的几种 `bad_params`；没在等的 `not_asking`，
//! 允许却带了理由的 `unexpected_reason`。
//!
//! 假工具 `edit` 只报要写的路径、不真写：报一条根目录下不存在的路径，它不在工作区、临时目录这些能写的地方，一定要问人
//! （测试的工作区都在系统的临时目录里，临时目录能写，写在那里不问）。

mod support;

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_kernel::event::{Body, ToolStatus};
use miyu_kernel::tool::Access;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use support::*;

/// 根目录下一个不存在的目录里的文件，写成她会给的样子。
fn nowhere(name: &str) -> String {
    let root = if cfg!(windows) { "C:\\" } else { "/" };
    let dir = format!("miyu-nowhere-{}", std::process::id());
    std::path::Path::new(root)
        .join(dir)
        .join(name)
        .to_string_lossy()
        .into_owned()
}

/// 她调一次 `edit`，写 `path`。
fn edit(path: &str) -> Play {
    Play::calls(&[("edit", &json!({ "path": path }).to_string())])
}

/// 起核心、握手、在工作区里造会话、说一句：交回连接、会话编号、假工具。
async fn started(home: &Home, plays: Vec<Play>) -> (Client, String, Arc<Fake>) {
    let fake = Fake::new("edit", Access::Write, Act::Echo);
    let tools = Catalog::new([Arc::clone(&fake) as Arc<dyn Tool>]).expect("合写法");
    let mut client = Client::connect(home.core_with_tools(&Script::new(plays), tools, TOKEN));
    client.hello().await;
    let cwd = home.work.to_string_lossy().into_owned();
    let created = client
        .call("create-1", "session.create", json!({ "cwd": cwd }))
        .await;
    let session = created["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("应该造出会话：{created}"))
        .to_string();
    client.say("send-1", &session, "hi").await;
    (client, session, fake)
}

/// 日志里的确认请求的调用编号，照先后。
fn requested(home: &Home, session: &str) -> Vec<String> {
    logged(home, session)
        .into_iter()
        .filter(|event| event["kind"] == json!("tool.approval_requested"))
        .map(|event| {
            event["body"]["call_id"]
                .as_str()
                .expect("有编号")
                .to_string()
        })
        .collect()
}

/// 等到日志里有 `n` 条确认请求，交回最后那一条的调用编号。
async fn nth_request(home: &Home, session: &str, n: usize) -> String {
    until("确认请求", || requested(home, session).len() >= n).await;
    requested(home, session).pop().expect("有")
}

async fn answer(client: &mut Client, id: &str, params: Value) -> Value {
    client.call(id, "session.answer", params).await
}

fn reason_of(reply: &Value) -> &Value {
    &reply["error"]["data"]["reason"]
}

#[tokio::test]
async fn allowing_once_runs_it_and_the_reply_lists_the_events() {
    let home = Home::new();
    let plays = vec![edit(&nowhere("a.txt")), Play::Says("好。")];
    let (mut client, session, fake) = started(&home, plays).await;
    let call = nth_request(&home, &session, 1).await;
    assert!(fake.calls().is_empty(), "还没回答，没跑");
    let params = json!({"session": session, "call": call, "decision": "once"});
    let reply = answer(&mut client, "answer-1", params).await;
    assert_eq!(
        reply["result"]["events"].as_array().map(Vec::len),
        Some(1),
        "{reply}"
    );
    home.until_turns(&session, 1).await;
    assert_eq!(fake.calls().len(), 1);
    let decided: Vec<Value> = logged(&home, &session)
        .into_iter()
        .filter(|event| event["kind"] == json!("tool.approval_decided"))
        .collect();
    assert_eq!(decided[0]["body"]["decision"], json!("once"));
    assert_eq!(decided[0]["cause"], json!("answer-1"));
}

#[tokio::test]
async fn allowing_for_the_session_stops_asking_about_that_directory() {
    let home = Home::new();
    let plays = vec![
        edit(&nowhere("a.txt")),
        edit(&nowhere("b.txt")),
        Play::Says("好。"),
    ];
    let (mut client, session, fake) = started(&home, plays).await;
    let call = nth_request(&home, &session, 1).await;
    let params = json!({"session": session, "call": call, "decision": "session"});
    let reply = answer(&mut client, "answer-1", params).await;
    assert!(reply["result"]["events"].is_array(), "{reply}");
    home.until_turns(&session, 1).await;
    assert_eq!(fake.calls().len(), 2, "第二次写同一个目录不再问");
    assert_eq!(requested(&home, &session).len(), 1);
}

#[tokio::test]
async fn a_denial_carries_the_reason_to_her_and_the_turn_goes_on() {
    let home = Home::new();
    let plays = vec![edit(&nowhere("a.txt")), Play::Says("那换个地方。")];
    let (mut client, session, fake) = started(&home, plays).await;
    let call = nth_request(&home, &session, 1).await;
    let params =
        json!({"session": session, "call": call, "decision": "deny", "reason": "别写根目录"});
    let reply = answer(&mut client, "answer-1", params).await;
    assert_eq!(
        reply["result"]["events"].as_array().map(Vec::len),
        Some(2),
        "{reply}"
    );
    home.until_turns(&session, 1).await;
    assert!(fake.calls().is_empty());
    let result = home
        .log(&session)
        .into_iter()
        .find_map(|event| match event.body {
            Body::ToolResult(result) => Some(result),
            _ => None,
        })
        .expect("有一条工具结果");
    assert_eq!(result.status, ToolStatus::Denied);
    let text = format!("{:?}", result.blocks);
    assert!(text.contains("别写根目录"), "{text}");
}

#[tokio::test]
async fn params_that_do_not_fit_are_bad_params() {
    let home = Home::new();
    let plays = vec![edit(&nowhere("a.txt")), Play::Says("好。")];
    let (mut client, session, _) = started(&home, plays).await;
    let call = nth_request(&home, &session, 1).await;
    let cases = [
        json!({"session": session, "call": call}),
        json!({"session": session, "call": call, "decision": "once", "answers": []}),
        json!({"session": session, "call": call, "decision": "workspace"}),
        json!({"session": session, "call": call, "decision": "ONCE"}),
        json!({"session": session, "call": call, "answers": [], "reason": "x"}),
        json!({"session": session, "call": "call-1", "decision": "once"}),
        json!({"session": "nope", "call": call, "decision": "once"}),
    ];
    for (k, params) in cases.into_iter().enumerate() {
        let reply = answer(&mut client, &format!("bad-{k}"), params.clone()).await;
        assert_eq!(
            reason_of(&reply),
            &json!("bad_params"),
            "{params} → {reply}"
        );
    }
    // 都没改动什么：还在等。
    assert!(
        !logged(&home, &session)
            .iter()
            .any(|event| event["kind"] == json!("tool.approval_decided"))
    );
}

#[tokio::test]
async fn the_kernel_refusals_come_back_with_their_reasons() {
    let home = Home::new();
    let plays = vec![edit(&nowhere("a.txt")), Play::Says("好。")];
    let (mut client, session, _) = started(&home, plays).await;
    let call = nth_request(&home, &session, 1).await;
    let params = json!({"session": session, "call": call, "decision": "once", "reason": "为什么"});
    let reply = answer(&mut client, "answer-1", params).await;
    assert_eq!(reason_of(&reply), &json!("unexpected_reason"), "{reply}");
    let params = json!({"session": session, "call": call, "decision": "once"});
    answer(&mut client, "answer-2", params.clone()).await;
    home.until_turns(&session, 1).await;
    // 答过了：不在等。
    let reply = answer(&mut client, "answer-3", params).await;
    assert_eq!(reason_of(&reply), &json!("not_asking"), "{reply}");
    // 测试的头握手时说中文。
    assert_eq!(
        reply["error"]["message"],
        json!("它没在等回答：已经答过，或者已经了结了。")
    );
}
