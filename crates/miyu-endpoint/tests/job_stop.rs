//! 协议上停掉一个任务（施工 7-4，`docs/blueprint/protocol.md` 的 `job.stop`）：真核心走一遍。人停子代理：子会话那一轮被打断，
//! 父会话记下 `child.reported`（`stopped`，不是她停的）；回应是 `{}`。没有这个任务、已经结束了的拒绝，`unknown_job`，中文、英文
//! 各一句；任务编号不合写法的参数不对。

mod support;

use serde_json::{Value, json};

use miyu_kernel::event::{Body, ChildReason, EndReason, Event};
use miyu_kernel::origin::{By, Session};
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;
use support::{Client, Home, TOKEN, default_resources, reason, until};

/// 父会话派出去的子会话的编号。
fn started_child(log: &[Event]) -> String {
    log.iter()
        .find_map(|event| match &event.body {
            Body::ToolResult(result) => result.effects.iter().find_map(|effect| match effect {
                miyu_kernel::event::Effect::JobStarted(started) => started
                    .session
                    .as_ref()
                    .map(|session| session.as_str().to_string()),
                _ => None,
            }),
            _ => None,
        })
        .expect("派出去了一个子会话")
}

#[tokio::test]
async fn a_person_stops_a_subagent() {
    let home = Home::new();
    let tools = Catalog::new(miyu_basesystem::tools(&default_resources()).unwrap()).unwrap();
    let args = json!({"description": "查 crate", "prompt": "Read Cargo.toml."}).to_string();
    // 派出去以后，父会话的下一次请求和子会话的第一次请求都停住：谁先到都一样。
    let script = Script::new([Play::calls(&[("agent", &args)]), Play::Holds, Play::Holds]);
    let mut client = Client::connect(home.core_with_tools(&script, tools, TOKEN));
    client.hello().await;
    let work = home.work.to_string_lossy().into_owned();
    let parent = client.create("c1", &work).await;
    client.say("c2", &parent, "派一个去查").await;
    until("两次请求都停住", || script.requests().len() == 3).await;
    let child = started_child(&home.log(&parent));

    let reply = client
        .call("s1", "job.stop", json!({"session": parent, "job": "j1"}))
        .await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    let log = home.log(&parent);
    let (event, reported) = log
        .iter()
        .find_map(|event| match &event.body {
            Body::ChildReported(reported) => Some((event, reported)),
            _ => None,
        })
        .expect("回应之前记下了回报");
    assert_eq!(reported.reason, ChildReason::Stopped);
    assert!(!reported.by_model, "人停的");
    assert_eq!(
        event.by,
        By::Session(Session {
            id: miyu_kernel::id::SessionId::parse(&child).unwrap()
        })
    );
    // 子会话：那一轮由父会话打断，停下了。
    let child_log = home.log(&child);
    let ended = child_log
        .iter()
        .find_map(|event| match &event.body {
            Body::TurnEnded(ended) => Some((event, ended)),
            _ => None,
        })
        .expect("子会话那一轮结束了");
    assert_eq!(ended.1.reason, EndReason::Interrupted);
    assert_eq!(
        ended
            .0
            .cause
            .as_ref()
            .map(|cause| cause.as_str().to_string()),
        Some(format!("{parent}/j1/stop"))
    );

    let again = client
        .call("s2", "job.stop", json!({"session": parent, "job": "j1"}))
        .await;
    assert_eq!(reason(&again), Some("unknown_job"), "停过的不再停：{again}");
}

#[tokio::test]
async fn what_is_not_there_is_refused() {
    let home = Home::new();
    let script = Script::new([]);
    let core = home.core(&script);
    let mut client = Client::connect(core.clone());
    client.hello().await;
    let session = client.create("c1", "~").await;
    let reply = client
        .call("s1", "job.stop", json!({"session": session, "job": "j1"}))
        .await;
    assert_eq!(reason(&reply), Some("unknown_job"), "{reply}");
    assert_eq!(
        reply["error"]["message"],
        json!("没有这个任务，或者它已经结束了。")
    );
    let mut english = Client::connect(core);
    english.hello_without_input().await;
    let reply = english
        .call("e1", "job.stop", json!({"session": session, "job": "j1"}))
        .await;
    assert_eq!(
        reply["error"]["message"],
        json!("There is no such job, or it has already ended.")
    );
    for job in [
        json!("1"),
        json!("j0"),
        json!("j1.0"),
        json!("j1."),
        json!(1),
        Value::Null,
    ] {
        let reply = client
            .call("s2", "job.stop", json!({"session": session, "job": job}))
            .await;
        assert_eq!(reason(&reply), Some("bad_params"), "{job}: {reply}");
    }
    let reply = client
        .call("s4", "job.stop", json!({"session": session, "job": "j1.1"}))
        .await;
    assert_eq!(
        reason(&reply),
        Some("unknown_job"),
        "几段的编号合写法，只是没有这个任务（施工 7-1 补）：{reply}"
    );
    let reply = client
        .call(
            "s3",
            "job.stop",
            json!({"session": "01a0d78c-ca52-7d19-8b64-000000000000", "job": "j1"}),
        )
        .await;
    assert_eq!(reason(&reply), Some("session_not_found"), "{reply}");
    assert_eq!(home.log(&session).len(), 1, "拒绝的什么都不写");
}
