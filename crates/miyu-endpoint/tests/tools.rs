//! 工具面（施工 4-1）：协议上造的会话，工具面照核心的工具目录存进策略快照。

mod support;

use std::sync::Arc;

use miyu_kernel::event::Body;
use miyu_kernel::tool::Access;
use miyu_policy::Snapshot;
use miyu_session::testkit::{Play, Script};
use miyu_store::blob::Blobs;
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};
use serde_json::json;
use support::{Client, Home, TOKEN, alice};

#[tokio::test]
async fn a_session_made_over_the_protocol_gets_the_cores_tools() {
    let home = Home::new();
    let read: Arc<dyn Tool> = Fake::new("read", Access::Read, Act::Echo);
    let tools = Catalog::new([read]).expect("合写法");
    let mut client = Client::connect(home.core_with_tools(&Script::new([]), tools, TOKEN));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let log = home.log(&session);
    let Body::SessionCreated(created) = &log[0].body else {
        panic!("第 1 条应该是造会话");
    };
    let bytes = Blobs::new(home.root.blobs(&alice()))
        .get(&created.policy)
        .expect("快照在 blob 里");
    let snapshot = Snapshot::from_bytes(&bytes).expect("读得懂");
    let names: Vec<&str> = snapshot
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect();
    assert_eq!(names, ["read"]);
    assert_eq!(snapshot.tools[0].access, Access::Read);
}

#[tokio::test]
async fn a_session_loaded_after_a_restart_runs_the_cores_tools() {
    let home = Home::new();
    let echo = Fake::new("echo", Access::Read, Act::Echo);
    let tools = || Catalog::new([Arc::clone(&echo) as Arc<dyn Tool>]).expect("合写法");
    let mut client = Client::connect(home.core_with_tools(&Script::new([]), tools(), TOKEN));
    client.hello().await;
    let session = client.create("c1", "~").await;
    // 换一份核心，像重启过：会话从磁盘载入，照新核心的目录执行工具（施工 4-2）。
    let script = Script::new([Play::Calls(&[("echo", "{}")]), Play::Says("好。")]);
    let mut client = Client::connect(home.core_with_tools(&script, tools(), TOKEN));
    client.hello().await;
    let reply = client
        .call(
            "c2",
            "session.send",
            json!({"session": session, "text": "hi"}),
        )
        .await;
    assert!(reply["result"]["events"].is_array(), "{reply}");
    home.until_turns(&session, 1).await;
    assert_eq!(echo.calls().len(), 1, "跑的是目录里的那一件");
}
