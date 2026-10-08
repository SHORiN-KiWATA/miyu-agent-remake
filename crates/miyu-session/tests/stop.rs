//! 叫停改文件的调用（施工 4-9 再补一）：打断时在跑的写，执行器只举旗、不掐任务，工具停在改之前或者做完，照常
//! 送回；又打断一次就不等了，掐掉它。

use std::sync::Arc;
use std::time::Duration;

use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, Event, ToolResult, ToolStatus};
use miyu_kernel::session::{Command, Queued};
use miyu_kernel::tool::Access;
use miyu_session::Handle;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::support::*;

/// 一件改文件的假工具、调它的会话：说一句，等它开始跑。
async fn writing(home: &Home, act: Act) -> (Arc<Fake>, Handle) {
    let tool = Fake::new("write", Access::Write, act);
    let catalog = Catalog::new([Arc::clone(&tool) as Arc<dyn Tool>]).expect("合写法");
    let script = Script::new([Play::calls(&[("write", "{}")])]);
    let handle = home.create_with(&script, &catalog).await;
    ask(&handle, "cmd-1", say("写一下"))
        .await
        .expect("会话在跑");
    until("工具开始跑", || tool.calls().len() == 1).await;
    (tool, handle)
}

/// 打断：排着队的退回来。
fn interrupt() -> Command {
    Command::Interrupt {
        queued: Queued::Return,
    }
}

/// 日志里唯一的那条工具结果。
fn result(log: &[Event]) -> ToolResult {
    let results: Vec<&ToolResult> = log
        .iter()
        .filter_map(|event| match &event.body {
            Body::ToolResult(result) => Some(result),
            _ => None,
        })
        .collect();
    assert_eq!(results.len(), 1, "{results:?}");
    results[0].clone()
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
async fn a_running_write_is_asked_to_stop_and_stops_before_changing() {
    let home = Home::new();
    let (tool, handle) = writing(&home, Act::Stops).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-2", interrupt()).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    assert!(tool.calls()[0].stop.stopped(), "叫它停就是举旗");
    assert_eq!(tool.dropped(), 0, "没掐：它自己交回来的");
    let result = result(&home.log(handle.id()));
    assert_eq!(result.status, ToolStatus::Cancelled);
    assert_eq!(
        text(&result.blocks),
        "The call was cancelled while it was running: the user interrupted the turn. It may have been partly done.\n"
    );
}

#[tokio::test]
async fn a_write_that_was_done_when_asked_to_stop_is_kept() {
    let home = Home::new();
    let (tool, handle) = writing(&home, Act::Finishes).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-2", interrupt()).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    assert_eq!(tool.dropped(), 0);
    let result = result(&home.log(handle.id()));
    assert_eq!(
        (result.status, text(&result.blocks)),
        (ToolStatus::Ok, "wrote".to_string())
    );
}

#[tokio::test]
async fn a_second_interrupt_cuts_off_the_write_that_does_not_stop() {
    let home = Home::new();
    let (tool, handle) = writing(&home, Act::Holds).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-2", interrupt()).await.expect("会话在跑");
    until("旗举起来", || tool.calls()[0].stop.stopped()).await;
    assert_eq!(tool.dropped(), 0, "叫它停只举旗，不掐");
    ask(&handle, "cmd-3", interrupt()).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    until("工具被掐掉", || tool.dropped() == 1).await;
    assert_eq!(result(&home.log(handle.id())).status, ToolStatus::Cancelled);
}
