//! 工具面（施工 4-1）：造会话时照工具目录存进快照，请求里的 tools 照名字排；载入以后照快照发，逐字节
//! 一样；没有工具的会话，请求里没有 tools。

mod support;

use std::sync::Arc;

use miyu_kernel::raw::RawJson;
use miyu_kernel::request::Request;
use miyu_kernel::tool::Access;
use miyu_session::testkit::{Play, Script};
use miyu_tool::{Catalog, Spec, Tool};

use support::*;

/// 一件只报规格的假工具。
struct Fake(Spec);

impl Tool for Fake {
    fn spec(&self) -> &Spec {
        &self.0
    }
}

fn fake(name: &str, access: Access, parameters: &str) -> Arc<dyn Tool> {
    Arc::new(Fake(Spec {
        name: name.to_string(),
        description: format!("The {name} tool."),
        parameters: serde_json::from_str::<RawJson>(parameters).expect("是 JSON"),
        access,
    }))
}

/// 两件假工具，登记的先后倒过来：read 在前、edit 在后。参数格式带着空白，原样进 tools。
fn two() -> Catalog {
    Catalog::new([
        fake(
            "read",
            Access::Read,
            r#"{ "type": "object", "properties": {"path": {"type": "string"}} }"#,
        ),
        fake("edit", Access::Write, r#"{"type":"object"}"#),
    ])
    .expect("两件都合写法")
}

fn names(request: &Request) -> Vec<&str> {
    request
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect()
}

/// 说一句 `words`，等这一轮说完，交回这一次的请求。
async fn one_turn(handle: &miyu_session::Handle, script: &Script, command: &str) -> Request {
    let mut pushes = watch(handle).await;
    let before = script.requests().len();
    ask(handle, command, say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let requests = script.requests();
    assert_eq!(requests.len(), before + 1, "这一轮请求了一次");
    requests.last().expect("有请求").1.clone()
}

#[tokio::test]
async fn the_request_carries_the_tools_by_name() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。")]);
    let handle = home.create_with(&script, &two()).await;
    let request = one_turn(&handle, &script, "cmd-1").await;
    assert_eq!(names(&request), ["edit", "read"]);
    assert_eq!(request.tools[0].description, "The edit tool.");
    assert_eq!(
        request.tools[1].parameters.get(),
        r#"{ "type": "object", "properties": {"path": {"type": "string"}} }"#
    );
}

#[tokio::test]
async fn a_loaded_session_sends_the_same_tools_and_system() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。")]);
    let handle = home.create_with(&script, &two()).await;
    let first = one_turn(&handle, &script, "cmd-1").await;
    let session = handle.id().clone();
    stop(&handle).await;
    let again = Script::new([Play::Says("还在。")]);
    let loaded = home.load(&session, &again).await;
    let second = one_turn(&loaded, &again, "cmd-2").await;
    assert_eq!(second.tools, first.tools);
    assert_eq!(second.system, first.system);
    assert_eq!(names(&second), ["edit", "read"]);
}

#[tokio::test]
async fn a_session_without_tools_sends_none() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。")]);
    let handle = home.create(&script).await;
    let request = one_turn(&handle, &script, "cmd-1").await;
    assert!(request.tools.is_empty());
}
