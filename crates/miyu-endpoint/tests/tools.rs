//! 工具面（施工 4-1）：协议上造的会话，工具面照核心的工具目录存进策略快照。

mod support;

use std::sync::Arc;

use miyu_kernel::event::Body;
use miyu_kernel::raw::RawJson;
use miyu_kernel::tool::Access;
use miyu_policy::Snapshot;
use miyu_session::testkit::Script;
use miyu_store::blob::Blobs;
use miyu_tool::{Catalog, Spec, Tool};
use support::{Client, Home, TOKEN, alice};

/// 一件只报规格的假工具。
struct Fake(Spec);

impl Tool for Fake {
    fn spec(&self) -> &Spec {
        &self.0
    }
}

#[tokio::test]
async fn a_session_made_over_the_protocol_gets_the_cores_tools() {
    let home = Home::new();
    let read: Arc<dyn Tool> = Arc::new(Fake(Spec {
        name: "read".to_string(),
        description: "The read tool.".to_string(),
        parameters: serde_json::from_str::<RawJson>(r#"{"type":"object"}"#).expect("是 JSON"),
        access: Access::Read,
    }));
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
