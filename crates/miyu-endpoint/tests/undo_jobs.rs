//! 撤销停掉那一轮派出去的，协议这一头（施工 7-8，`docs/blueprint/protocol/undo.md` 的 `jobs`）：真核心走一遍。撤销的回应
//! 列出停掉的任务（编号、种类、标题），回应之前后台命令已经整组杀了，回报记 `undone`；恢复撤销的回应不带这一格，停掉的
//! 不再起来；再撤销时已经结束了的不再列；重做的回应也列。

use std::path::Path;
use std::sync::Arc;

use serde_json::{Value, json};

use miyu_kernel::event::{Body, JobReason};
use miyu_kernel::tool::Access;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake, Held};
use miyu_tool::{Catalog, Tool};

use crate::support::*;

/// 基础系统的工具，加一件假的 `start`：把 `held` 交给任务端口，标题是 `fake`。
fn tools(held: &Arc<Held>) -> Catalog {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    let mut tools = miyu_basesystem::tools(&resources).expect("出厂的资源读得出来");
    let start: Arc<dyn Tool> = Fake::new("start", Access::Read, Act::Background(Arc::clone(held)));
    tools.push(start);
    Catalog::new(tools).expect("合写法")
}

/// 回应里的 `result`。
fn result_of(reply: &Value) -> &Value {
    assert!(reply.get("error").is_none(), "{reply}");
    &reply["result"]
}

/// 日志里 `undone` 的后台命令结束有几条。
fn undone(home: &Home, session: &str) -> usize {
    home.log(session)
        .iter()
        .filter(|event| {
            matches!(&event.body, Body::JobReported(reported) if reported.reason == JobReason::Undone)
        })
        .count()
}

#[tokio::test]
async fn an_undo_lists_the_jobs_it_stopped() {
    let home = Home::new();
    let held = Held::new(&["x\n"]);
    let plays = vec![Play::calls(&[("start", "{}")]), Play::Says("放出去了。")];
    let mut client =
        Client::connect(home.core_with_tools(&Script::new(plays), tools(&held), TOKEN));
    client.hello().await;
    let session = client.create("c1", &home.work.to_string_lossy()).await;
    client.say("c2", &session, "后台跑").await;
    home.until_turns(&session, 1).await;

    let reply = client
        .call("u1", "session.revert", json!({"session": session}))
        .await;
    assert_eq!(
        result_of(&reply)["jobs"],
        json!([{"job": "j1", "what": "command", "title": "fake"}]),
        "{reply}"
    );
    assert_eq!(held.killed(), 1, "回应之前已经整组杀了");
    until("撤销停掉的回报落盘", || {
        undone(&home, &session) == 1
    })
    .await;

    let reply = client
        .call("u2", "session.unrevert", json!({"session": session}))
        .await;
    assert!(result_of(&reply).get("jobs").is_none(), "恢复不带：{reply}");
    let reply = client
        .call("u3", "session.revert", json!({"session": session}))
        .await;
    assert!(
        result_of(&reply).get("jobs").is_none(),
        "已经停掉了，再撤销不再列：{reply}"
    );
    assert_eq!((held.killed(), undone(&home, &session)), (1, 1));
}

#[tokio::test]
async fn a_redo_lists_them_too() {
    let home = Home::new();
    let held = Held::new(&[]);
    let plays = vec![
        Play::calls(&[("start", "{}")]),
        Play::Says("放出去了。"),
        Play::Says("再来一次。"),
    ];
    let mut client =
        Client::connect(home.core_with_tools(&Script::new(plays), tools(&held), TOKEN));
    client.hello().await;
    let session = client.create("c1", &home.work.to_string_lossy()).await;
    client.say("c2", &session, "后台跑").await;
    home.until_turns(&session, 1).await;
    let reply = client
        .call("r1", "session.redo", json!({"session": session}))
        .await;
    assert_eq!(
        result_of(&reply)["jobs"],
        json!([{"job": "j1", "what": "command", "title": "fake"}]),
        "{reply}"
    );
    until("撤销停掉的回报落盘", || {
        undone(&home, &session) == 1
    })
    .await;
}
