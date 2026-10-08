//! 工具面（施工 4-1）和执行工具（施工 4-2）：造会话时照工具目录存下工具面，请求里的 tools 照名字排、
//! 载入以后逐字节一样；她调工具时，会话照名字在目录里找到那一件，在这一轮的工作目录里跑，结果落盘，
//! 她接着说。

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Barrier;

use miyu_kernel::block::{Block, Image, Text};
use miyu_kernel::event::{Body, Event, Said, ToolResult, ToolStatus, TransientBody};
use miyu_kernel::id::{ContentHash, MediaType};
use miyu_kernel::raw::RawJson;
use miyu_kernel::request::{Message, Request};
use miyu_kernel::session::{Command, Queued};
use miyu_kernel::tool::Access;
use miyu_session::testkit::{Play, Script};
use miyu_session::{Handle, Pushed};
use miyu_store::blob::Blobs;
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Spec, Tool};

use crate::support::*;

/// 这几件假工具的目录。
fn catalog(tools: &[&Arc<Fake>]) -> Catalog {
    Catalog::new(tools.iter().map(|tool| Arc::clone(tool) as Arc<dyn Tool>)).expect("都合写法")
}

/// 两件假工具，登记的先后倒过来：read 在前、edit 在后。参数格式带着空白，原样进 tools。
fn two() -> Catalog {
    let read = Fake::with_parameters(
        "read",
        Access::Read,
        r#"{ "type": "object", "properties": {"path": {"type": "string"}} }"#,
        Act::Echo,
    );
    let edit = Fake::new("edit", Access::Write, Act::Echo);
    catalog(&[&read, &edit])
}

fn names(request: &Request) -> Vec<&str> {
    request
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect()
}

/// 说一句，等这一轮说完，交回这一轮推过来的。
async fn turn(handle: &Handle, command: &str) -> Vec<Arc<Pushed>> {
    let mut pushes = watch(handle).await;
    ask(handle, command, say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await
}

/// 日志里的工具结果，照先后。
fn results(log: &[Event]) -> Vec<&ToolResult> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::ToolResult(result) => Some(result),
            _ => None,
        })
        .collect()
}

/// 一块字的内容。
fn text(blocks: &[Block]) -> String {
    blocks
        .iter()
        .map(|block| match block {
            Block::Text(text) => text.text.as_str(),
            other => panic!("只该有字：{other:?}"),
        })
        .collect()
}

/// 等到 `what` 为真，最多十秒。
async fn until(what: &str, mut ready: impl FnMut() -> bool) {
    within(what, async {
        while !ready() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
}

#[tokio::test]
async fn the_request_carries_the_tools_by_name() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。")]);
    let handle = home.create_with(&script, &two()).await;
    turn(&handle, "cmd-1").await;
    let request = script.requests()[0].1.clone();
    assert_eq!(names(&request), ["edit", "read"]);
    assert_eq!(request.tools[0].description, "The edit tool.");
    assert_eq!(
        request.tools[1].parameters.get(),
        r#"{ "type": "object", "properties": {"path": {"type": "string"}} }"#
    );
}

#[tokio::test]
async fn a_loaded_session_sends_the_tools_from_its_snapshot() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。")]);
    let handle = home.create_with(&script, &two()).await;
    turn(&handle, "cmd-1").await;
    let first = script.requests()[0].1.clone();
    let session = handle.id().clone();
    stop(&handle).await;
    // 核心升级了：同名的工具改了说明。载入的老会话照样发当时的工具面。
    let changed = Fake::from_spec(
        Spec {
            name: "read".to_string(),
            description: "Changed.".to_string(),
            parameters: serde_json::from_str::<RawJson>(r#"{"type":"object"}"#).expect("是 JSON"),
            access: Access::Read,
        },
        Act::Echo,
    );
    let again = Script::new([Play::Says("还在。")]);
    let loaded = home
        .load_with(&session, &again, &catalog(&[&changed]))
        .await;
    turn(&loaded, "cmd-2").await;
    let second = again.requests()[0].1.clone();
    assert_eq!(second.tools, first.tools);
    assert_eq!(second.system, first.system);
}

#[tokio::test]
async fn a_session_without_tools_sends_none() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。")]);
    let handle = home.create(&script).await;
    turn(&handle, "cmd-1").await;
    assert!(script.requests()[0].1.tools.is_empty());
}

#[tokio::test]
async fn a_call_runs_in_the_turns_directory_and_she_hears_the_result() {
    let home = Home::new();
    let echo = Fake::new("echo", Access::Read, Act::Echo);
    let script = Script::new([Play::calls(&[("echo", r#"{"x":1}"#)]), Play::Says("好。")]);
    let handle = home.create_with(&script, &catalog(&[&echo])).await;
    turn(&handle, "cmd-1").await;
    let calls = echo.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].args, r#"{"x":1}"#);
    assert_eq!(calls[0].cwd, environment().cwd);
    let log = home.log(handle.id());
    let results = results(&log);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].status, ToolStatus::Ok);
    assert!(results[0].duration_ms.is_some());
    let said = format!("cwd={} args={{\"x\":1}}", environment().cwd);
    assert_eq!(text(&results[0].blocks), said);
    // 下一次请求里有这个结果，她接着说完了。
    let requests = script.requests();
    assert_eq!(requests.len(), 2);
    let heard = requests[1].1.messages.iter().any(|message| {
        matches!(message, Message::Tool { error: false, blocks, .. } if text(blocks) == said)
    });
    assert!(heard, "{:#?}", requests[1].1.messages);
}

#[tokio::test]
async fn a_picture_from_a_tool_becomes_an_image_block_with_its_blob_stored() {
    // 工具交回的图片（施工 4-13）：存成属主的 blob，换成图片块接在字后面；下一次请求里带着它。
    let home = Home::new();
    let look = Fake::new("look", Access::Read, Act::Shows(b"fake png bytes"));
    let script = Script::new([Play::calls(&[("look", "{}")]), Play::Says("看到了。")]);
    let handle = home.create_with(&script, &catalog(&[&look])).await;
    turn(&handle, "cmd-1").await;
    let log = home.log(handle.id());
    let results = results(&log);
    assert_eq!(results.len(), 1);
    let hash = ContentHash::of(b"fake png bytes");
    let expected = vec![
        Block::Text(Text {
            text: "shown".to_string(),
        }),
        Block::Image(Image {
            blob: hash.clone(),
            name: None,
            media_type: MediaType::parse("image/png").unwrap(),
            width: 2,
            height: 1,
        }),
    ];
    assert_eq!(results[0].blocks, expected);
    let stored = Blobs::new(home.root.blobs(&alice_account()));
    assert_eq!(stored.get(&hash).unwrap(), b"fake png bytes");
    let requests = script.requests();
    let heard = requests[1]
        .1
        .messages
        .iter()
        .any(|message| matches!(message, Message::Tool { blocks, .. } if *blocks == expected));
    assert!(heard, "{:#?}", requests[1].1.messages);
}

#[tokio::test]
async fn a_tool_that_fails_gives_an_error_result() {
    let home = Home::new();
    let broken = Fake::new("read", Access::Read, Act::Fails("No such file: a.txt"));
    let script = Script::new([Play::calls(&[("read", "{}")]), Play::Says("好。")]);
    let handle = home.create_with(&script, &catalog(&[&broken])).await;
    turn(&handle, "cmd-1").await;
    let log = home.log(handle.id());
    let results = results(&log);
    assert_eq!(results[0].status, ToolStatus::Error);
    assert_eq!(text(&results[0].blocks), "No such file: a.txt");
}

#[tokio::test]
async fn the_output_while_running_is_pushed_but_not_stored() {
    let home = Home::new();
    let talker = Fake::new("run", Access::Read, Act::Pushes(&["one ", "two"]));
    let script = Script::new([Play::calls(&[("run", "{}")]), Play::Says("好。")]);
    let handle = home.create_with(&script, &catalog(&[&talker])).await;
    let pushed = turn(&handle, "cmd-1").await;
    let mut seen = Vec::new();
    for pushed in &pushed {
        match &**pushed {
            Pushed::Transient(transient) => {
                if let TransientBody::ToolProgress(progress) = &transient.body {
                    seen.push(format!("progress {}", progress.text));
                }
            }
            Pushed::Events(events) => {
                for event in events {
                    if let Body::ToolResult(result) = &event.body {
                        seen.push(format!("result {}", text(&result.blocks)));
                    }
                }
            }
        }
    }
    assert_eq!(seen, ["progress one ", "progress two", "result pushed"]);
    let log = home.log(handle.id());
    assert!(
        !kinds(&log).iter().any(|kind| kind.contains("progress")),
        "执行中的输出不落盘：{:?}",
        kinds(&log)
    );
}

#[tokio::test]
async fn two_read_only_calls_run_at_the_same_time() {
    let home = Home::new();
    // 两件都等凑齐两个才往下走：一件跑完了才派下一件的话，谁都走不完。
    let barrier = Arc::new(Barrier::new(2));
    let one = Fake::new("one", Access::Read, Act::Meets(Arc::clone(&barrier)));
    let two = Fake::new("two", Access::Read, Act::Meets(barrier));
    let script = Script::new([
        Play::calls(&[("one", "{}"), ("two", "{}")]),
        Play::Says("好。"),
    ]);
    let handle = home.create_with(&script, &catalog(&[&one, &two])).await;
    turn(&handle, "cmd-1").await;
    let log = home.log(handle.id());
    let results = results(&log);
    assert_eq!(results.len(), 2);
    assert!(results.iter().all(|result| text(&result.blocks) == "met"));
}

#[tokio::test]
async fn an_interrupt_drops_the_running_tool() {
    let home = Home::new();
    let stuck = Fake::new("wait", Access::Read, Act::Holds);
    let script = Script::new([Play::calls(&[("wait", "{}")])]);
    let handle = home.create_with(&script, &catalog(&[&stuck])).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until("工具开始跑", || stuck.calls().len() == 1).await;
    ask(
        &handle,
        "cmd-2",
        Command::Interrupt {
            queued: Queued::Return,
        },
    )
    .await
    .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    until("工具被丢掉", || stuck.dropped() == 1).await;
    assert!(stuck.calls()[0].stop.stopped(), "掐掉时旗也举起来");
    let log = home.log(handle.id());
    let results = results(&log);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].status, ToolStatus::Cancelled);
}

#[tokio::test]
async fn a_tool_the_catalog_no_longer_has_is_not_available() {
    let home = Home::new();
    let gone = Fake::new("gone", Access::Read, Act::Echo);
    let script = Script::new([Play::Says("好。")]);
    let handle = home.create_with(&script, &catalog(&[&gone])).await;
    turn(&handle, "cmd-1").await;
    let session = handle.id().clone();
    stop(&handle).await;
    // 核心升级拿掉了这件：快照里还有，她照样会调。
    let again = Script::new([Play::calls(&[("gone", "{}")]), Play::Says("好。")]);
    let loaded = home.load_with(&session, &again, &Catalog::default()).await;
    turn(&loaded, "cmd-2").await;
    assert!(gone.calls().is_empty());
    let log = home.log(&session);
    let results = results(&log);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].status, ToolStatus::Error);
    assert_eq!(results[0].duration_ms, None, "没跑过");
    assert_eq!(
        text(&results[0].blocks),
        "The tool \"gone\" is not available right now.\n"
    );
    assert_eq!(
        results[0].human,
        Some(Said::new("core/tool-results/unavailable").with("name", "gone"))
    );
}

#[tokio::test]
async fn a_tool_that_panics_gives_an_error_and_the_session_goes_on() {
    let home = Home::new();
    let buggy = Fake::new("boom", Access::Read, Act::Panics);
    let script = Script::new([
        Play::calls(&[("boom", "{}")]),
        Play::Says("好。"),
        Play::Says("在。"),
    ]);
    let handle = home.create_with(&script, &catalog(&[&buggy])).await;
    turn(&handle, "cmd-1").await;
    let log = home.log(handle.id());
    let results = results(&log);
    assert_eq!(results[0].status, ToolStatus::Error);
    assert_eq!(
        text(&results[0].blocks),
        "The tool \"boom\" stopped because of an internal error. It may have been partly done.\n"
    );
    assert_eq!(
        results[0].human,
        Some(Said::new("core/tool-results/crashed").with("name", "boom"))
    );
    assert_eq!(script.requests().len(), 2, "会话照常请求了下一次");
    // 会话还活着，接着说得上话。
    turn(&handle, "cmd-2").await;
    assert_eq!(script.requests().len(), 3);
}

#[tokio::test]
async fn stopping_the_session_drops_the_running_tools() {
    let home = Home::new();
    let stuck = Fake::new("wait", Access::Read, Act::Holds);
    let script = Script::new([Play::calls(&[("wait", "{}")])]);
    let handle = home.create_with(&script, &catalog(&[&stuck])).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until("工具开始跑", || stuck.calls().len() == 1).await;
    // 有计划地停下：内核先叫停在跑的工具。
    stop(&handle).await;
    drop(handle);
    until("工具被丢掉", || stuck.dropped() == 1).await;
}

#[tokio::test]
async fn a_session_nobody_holds_drops_the_running_tools() {
    let home = Home::new();
    let stuck = Fake::new("wait", Access::Read, Act::Holds);
    let script = Script::new([Play::calls(&[("wait", "{}")])]);
    let handle = home.create_with(&script, &catalog(&[&stuck])).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until("工具开始跑", || stuck.calls().len() == 1).await;
    // 没人拿着了：actor 自己退出，不经过有计划的停下，内核也就不会先叫停工具。
    drop(handle);
    until("工具被丢掉", || stuck.dropped() == 1).await;
}
